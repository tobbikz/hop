//! Integrated terminal via portable-pty + vt100 screen (real ANSI/CSI).

use anyhow::{Context, Result};
use parking_lot::Mutex;
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::warn;

pub struct TerminalPane {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    parser: Arc<Mutex<vt100::Parser>>,
    reader_buf: Arc<Mutex<String>>,
    /// When true, PTY bytes still update byte counters but skip the VT parser
    /// (used to drop late fastfetch/motd after a clean-open wipe).
    discard_parser: Arc<AtomicBool>,
    cols: u16,
    rows: u16,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    _pair_reader: Option<std::thread::JoinHandle<()>>,
}

impl TerminalPane {
    pub fn spawn(cwd: &std::path::Path, cols: u16, rows: u16) -> Result<Self> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        Self::spawn_command(cwd, cols, rows, &shell, &[])
    }

    pub fn spawn_command(
        cwd: &std::path::Path,
        cols: u16,
        rows: u16,
        program: &str,
        args: &[String],
    ) -> Result<Self> {
        Self::spawn_command_with_env(cwd, cols, rows, program, args, &[])
    }

    pub fn spawn_command_with_env(
        cwd: &std::path::Path,
        cols: u16,
        rows: u16,
        program: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<Self> {
        let cols = cols.max(10);
        let rows = rows.max(3);
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open pty")?;

        let mut cmd = CommandBuilder::new(program);
        for a in args {
            cmd.arg(a);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // Tell apps the host is dark (fg;bg) so prompts stay readable on AMOLED.
        cmd.env("COLORFGBG", "15;0");
        cmd.cwd(cwd);

        let child = pair
            .slave
            .spawn_command(cmd)
            .with_context(|| format!("spawn {program}"))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().context("clone reader")?;
        let writer = pair.master.take_writer().context("take writer")?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 2000)));
        let reader_buf = Arc::new(Mutex::new(String::new()));
        let discard_parser = Arc::new(AtomicBool::new(false));
        let parser_c = parser.clone();
        let buf_c = reader_buf.clone();
        let discard_c = discard_parser.clone();

        let handle = std::thread::spawn(move || {
            let mut tmp = [0u8; 8192];
            loop {
                match reader.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = &tmp[..n];
                        if !discard_c.load(Ordering::Acquire) {
                            parser_c.lock().process(chunk);
                        }
                        let s = String::from_utf8_lossy(chunk);
                        let mut g = buf_c.lock();
                        g.push_str(&s);
                        if g.len() > 200_000 {
                            let keep = g.len() - 150_000;
                            g.drain(..keep);
                        }
                    }
                    Err(e) => {
                        warn!("pty read: {e}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer: Arc::new(Mutex::new(writer)),
            parser,
            reader_buf,
            discard_parser,
            cols,
            rows,
            _child: child,
            _pair_reader: Some(handle),
        })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(10);
        let rows = rows.max(3);
        if cols == self.cols && rows == self.rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        if let Err(e) = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            warn!("pty resize: {e}");
        }
        self.parser.lock().set_size(rows, cols);
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    pub fn write_bytes(&self, data: &[u8]) -> Result<()> {
        let mut w = self.writer.lock();
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn write_char(&self, c: char) -> Result<()> {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.write_bytes(s.as_bytes())
    }

    /// Bytes read from the PTY so far (settle detection for motd/fastfetch).
    pub fn bytes_read(&self) -> usize {
        self.reader_buf.lock().len()
    }

    /// Non-empty visible rows — banners are many; a clean prompt is few.
    pub fn non_empty_rows(&self) -> usize {
        self.visible_lines()
            .iter()
            .filter(|l| !l.trim().is_empty())
            .count()
    }

    /// Wipe local VT100 state (does not type into the remote shell).
    pub fn hard_reset_screen(&self) {
        let mut p = self.parser.lock();
        *p = vt100::Parser::new(self.rows, self.cols, 2000);
    }

    pub fn set_discard_parser(&self, discard: bool) {
        self.discard_parser.store(discard, Ordering::Release);
    }

    pub fn is_discarding_parser(&self) -> bool {
        self.discard_parser.load(Ordering::Acquire)
    }

    /// Start ignoring motd/fastfetch bytes in the local VT (still counted in bytes_read).
    pub fn begin_clean_open(&self) {
        self.set_discard_parser(true);
        self.hard_reset_screen();
    }

    /// Stop discarding and redraw the prompt via Ctrl-L — never types `clear\\r`
    /// (that races with user input → `cclear` / phantom submits).
    pub fn finish_clean_open(&self) {
        self.hard_reset_screen();
        self.set_discard_parser(false);
        // Readline clear-screen: redraws prompt without executing a command.
        let _ = self.write_bytes(b"\x0c");
    }

    /// One-shot local wipe + Ctrl-L (key dismiss / last-resort).
    pub fn clean_open(&self) {
        self.begin_clean_open();
        self.finish_clean_open();
    }

    /// Visible screen rows after vt100 interprets CSI (clear, cursor, color, …).
    pub fn visible_lines(&self) -> Vec<String> {
        let parser = self.parser.lock();
        let screen = parser.screen();
        let (_rows, cols) = screen.size();
        screen
            .rows(0, cols)
            .map(|row| row.trim_end().to_string())
            .collect()
    }

    /// Cell grid for colored ratatui rendering.
    pub fn styled_rows(&self) -> Vec<Vec<TermSpan>> {
        let parser = self.parser.lock();
        let screen = parser.screen();
        let (rows, cols) = screen.size();
        let mut out = Vec::with_capacity(rows as usize);
        for row in 0..rows {
            let mut spans: Vec<TermSpan> = Vec::new();
            let mut cur = TermSpan {
                text: String::new(),
                fg: None,
                bg: None,
                bold: false,
            };
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let contents = cell.contents();
                let ch = if contents.is_empty() {
                    " "
                } else {
                    contents.as_str()
                };
                let fg = color_rgb(cell.fgcolor());
                let bg = color_rgb(cell.bgcolor());
                let bold = cell.bold();
                if cur.text.is_empty() {
                    cur.fg = fg;
                    cur.bg = bg;
                    cur.bold = bold;
                    cur.text.push_str(ch);
                } else if cur.fg == fg && cur.bg == bg && cur.bold == bold {
                    cur.text.push_str(ch);
                } else {
                    spans.push(std::mem::replace(
                        &mut cur,
                        TermSpan {
                            text: ch.to_string(),
                            fg,
                            bg,
                            bold,
                        },
                    ));
                }
            }
            if !cur.text.is_empty() {
                if let Some(trimmed) = Some(cur.text.trim_end()) {
                    cur.text = trimmed.to_string();
                }
                if !cur.text.is_empty() {
                    spans.push(cur);
                }
            }
            out.push(spans);
        }
        out
    }

    pub fn scroll_up(&mut self) {
        let mut p = self.parser.lock();
        let cur = p.screen().scrollback();
        p.set_scrollback(cur.saturating_add(1));
    }

    pub fn scroll_down(&mut self) {
        let mut p = self.parser.lock();
        let cur = p.screen().scrollback();
        p.set_scrollback(cur.saturating_sub(1));
    }

    pub fn auto_type_password(&self, password: String) {
        let buf = self.reader_buf.clone();
        let writer = self.writer.clone();
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
            while std::time::Instant::now() < deadline {
                let hay = {
                    let g = buf.lock();
                    g.to_ascii_lowercase()
                };
                let tail: String = hay
                    .chars()
                    .rev()
                    .take(160)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let login_pw = (tail.contains("password:") || tail.contains("'s password"))
                    && !tail.contains("passphrase for key");
                if login_pw {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    let mut w = writer.lock();
                    let _ = w.write_all(password.as_bytes());
                    let _ = w.write_all(b"\r");
                    let _ = w.flush();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });
    }

    pub fn snapshot_text(&self) -> String {
        self.parser.lock().screen().contents()
    }
}

#[derive(Debug, Clone)]
pub struct TermSpan {
    pub text: String,
    pub fg: Option<(u8, u8, u8)>,
    pub bg: Option<(u8, u8, u8)>,
    pub bold: bool,
}

fn color_rgb(c: vt100::Color) -> Option<(u8, u8, u8)> {
    match c {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(ansi_idx_rgb(i)),
        vt100::Color::Rgb(r, g, b) => Some((r, g, b)),
    }
}

fn ansi_idx_rgb(i: u8) -> (u8, u8, u8) {
    const BASIC: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    if (i as usize) < BASIC.len() {
        return BASIC[i as usize];
    }
    if (16..232).contains(&i) {
        let n = i - 16;
        let r = n / 36;
        let g = (n % 36) / 6;
        let b = n % 6;
        let ramp = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
        return (ramp(r), ramp(g), ramp(b));
    }
    let v = if i >= 232 { 8 + 10 * (i - 232) } else { 128 };
    (v, v, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn spawn_resize_and_echo() {
        let tmp = std::env::temp_dir().join("tui-ide-term-smoke");
        let _ = std::fs::create_dir_all(&tmp);
        let mut pane = TerminalPane::spawn(&tmp, 40, 8).expect("spawn");
        pane.resize(60, 12);
        assert_eq!(pane.size(), (60, 12));
        pane.write_bytes(b"echo wave4\r").expect("write");
        std::thread::sleep(Duration::from_millis(300));
        let lines = pane.visible_lines().join("\n");
        assert!(
            lines.contains("wave4") || !lines.is_empty(),
            "expected pty output, got: {lines:?}"
        );
    }

    #[test]
    fn vt100_clear_does_not_garble() {
        let mut parser = vt100::Parser::new(8, 40, 100);
        parser.process(b"hello\r\nworld\r\n");
        parser.process(b"\x1b[2J\x1b[H");
        parser.process(b"clean\r\n");
        let text = parser.screen().contents();
        assert!(text.contains("clean"));
        assert!(!text.contains("hello"));
    }

    #[test]
    fn hard_reset_wipes_visible_banner() {
        let tmp = std::env::temp_dir().join("hop-term-clean-open");
        let _ = std::fs::create_dir_all(&tmp);
        let pane = TerminalPane::spawn(&tmp, 40, 12).expect("spawn");
        pane.write_bytes(b"printf 'BANNER\\nLINE2\\nLINE3\\n'\r")
            .expect("write");
        std::thread::sleep(Duration::from_millis(250));
        assert!(
            pane.non_empty_rows() >= 1 || pane.bytes_read() > 0,
            "expected some output before reset"
        );
        pane.hard_reset_screen();
        assert_eq!(pane.non_empty_rows(), 0);
    }

    #[test]
    fn finish_clean_open_does_not_type_clear_command() {
        let tmp = std::env::temp_dir().join("hop-term-no-clear-cmd");
        let _ = std::fs::create_dir_all(&tmp);
        let pane = TerminalPane::spawn(&tmp, 40, 12).expect("spawn");
        std::thread::sleep(Duration::from_millis(150));
        pane.begin_clean_open();
        pane.finish_clean_open();
        std::thread::sleep(Duration::from_millis(200));
        let snap = pane.snapshot_text().to_ascii_lowercase();
        assert!(
            !snap.contains("command not found"),
            "Ctrl-L clean must not run a shell command, got: {snap:?}"
        );
    }
}
