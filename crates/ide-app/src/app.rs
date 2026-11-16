//! Application loop: session input, PTY rendering, overlays.

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ide_config::IdeConfig;
use ide_core::{Focus, Mode};
use ide_sessions::{ConnectOutcome, SessionKind, SessionManager};
use ide_term::TerminalPane;
use ide_ui::theme;
use ide_ui::{render, Palette, RenderCtx, StatusMessage, UiState};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct App {
    ui: UiState,
    sessions: SessionManager,
    config: IdeConfig,
    cwd: PathBuf,
    should_quit: bool,
    status_at: Option<Instant>,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut config = IdeConfig::load();
        if config.theme.name != theme::LOCKED_THEME_ID {
            config.theme.name = theme::LOCKED_THEME_ID.to_string();
            let _ = config.save();
        }
        theme::reload_customs();
        theme::set_mode_name(theme::LOCKED_THEME_ID);

        let mut sessions = SessionManager::load();
        sessions.reap_rdp();

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut app = Self {
            ui: UiState::default(),
            sessions,
            config,
            cwd,
            should_quit: false,
            status_at: None,
        };
        app.set_status(
            "Sessions · Enter connect · a SSH · A RDP · e edit · x del",
            false,
        );
        Ok(app)
    }

    fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.ui.status = Some(StatusMessage {
            text: text.into(),
            error,
        });
        self.status_at = Some(Instant::now());
    }

    fn expire_status(&mut self) {
        if let Some(at) = self.status_at {
            if at.elapsed() >= Duration::from_secs(4) {
                self.ui.status = None;
                self.status_at = None;
            }
        }
    }

    fn handle_event(&mut self, event: Event, term_rows: u16) -> Result<()> {
        if let Event::Key(key) = event {
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                self.handle_key(key, term_rows)?;
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent, term_rows: u16) -> Result<()> {
        let in_pty = self.ui.mode == Mode::Normal && self.ui.focus == Focus::Terminal;
        if !in_pty
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            self.should_quit = true;
            return Ok(());
        }
        match self.ui.mode {
            Mode::Palette => self.handle_palette_key(key, term_rows)?,
            Mode::SessionForm => self.handle_form_key(key)?,
            Mode::SessionPassword => self.handle_password_key(key, term_rows)?,
            Mode::ConfirmDelete => self.handle_confirm_key(key),
            Mode::Normal => {
                if self.ui.focus == Focus::Terminal {
                    self.handle_terminal_key(key)?;
                } else {
                    self.handle_sessions_key(key, term_rows)?;
                }
            }
        }
        Ok(())
    }

    fn handle_palette_key(&mut self, key: KeyEvent, term_rows: u16) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.close_palette(),
            KeyCode::Enter => self.run_palette_selected(term_rows),
            KeyCode::Up => self.ui.palette.move_up(),
            KeyCode::Down => self.ui.palette.move_down(),
            KeyCode::Backspace => self.ui.palette.backspace(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.ui.palette.push_char(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.ui.mode = Mode::Normal;
                self.ui.focus = Focus::Sessions;
                self.set_status("profile edit cancelled", false);
            }
            KeyCode::Enter => match self.sessions.commit_form() {
                Ok(()) => {
                    self.ui.mode = Mode::Normal;
                    self.ui.focus = Focus::Sessions;
                    self.set_status("profile saved", false);
                }
                Err(e) => self.set_status(format!("session form: {e}"), true),
            },
            KeyCode::Tab | KeyCode::Down => {
                self.ui.session_form_field = (self.ui.session_form_field + 1) % 6;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.ui.session_form_field = if self.ui.session_form_field == 0 {
                    5
                } else {
                    self.ui.session_form_field - 1
                };
            }
            KeyCode::Backspace => {
                self.session_form_field_mut().pop();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.session_form_field_mut().push(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_password_key(&mut self, key: KeyEvent, term_rows: u16) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.ui.session_password_input.clear();
                self.ui.session_password_profile_id = None;
                self.ui.mode = Mode::Normal;
                self.ui.focus = Focus::Sessions;
                self.set_status("connect cancelled", false);
            }
            KeyCode::Enter => self.confirm_session_password(term_rows)?,
            KeyCode::Backspace => {
                self.ui.session_password_input.pop();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.ui.session_password_remember = !self.ui.session_password_remember;
                let mark = if self.ui.session_password_remember {
                    "remember ON"
                } else {
                    "remember OFF"
                };
                self.set_status(format!("password · {mark}"), false);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.ui.session_password_input.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_sessions_key(&mut self, key: KeyEvent, term_rows: u16) -> Result<()> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.sessions.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.sessions.move_up(),
            KeyCode::Enter => self.begin_session_connect(term_rows),
            KeyCode::Char('a') if !shift => self.begin_add(true),
            KeyCode::Char('A') => self.begin_add(false),
            KeyCode::Char('e') => match self.sessions.begin_edit_selected() {
                Ok(()) => {
                    self.ui.session_form_field = 0;
                    self.ui.mode = Mode::SessionForm;
                }
                Err(e) => self.set_status(e.to_string(), true),
            },
            KeyCode::Char('x') | KeyCode::Delete => {
                if self.sessions.selected_profile().is_some() {
                    self.ui.show_discover = false;
                    self.ui.mode = Mode::ConfirmDelete;
                } else {
                    self.set_status("no profile selected", true);
                }
            }
            KeyCode::Char(']') => self.next_ssh_tab(),
            KeyCode::Char('[') => self.prev_ssh_tab(),
            KeyCode::Char('w') => self.close_ssh_tab(),
            KeyCode::Char(' ') if !ctrl => {
                self.ui.show_discover = !self.ui.show_discover;
            }
            KeyCode::Char('T') if shift => self.apply_locked_theme(),
            KeyCode::Char('T') => self.apply_locked_theme(),
            KeyCode::Char('p') if ctrl => self.open_palette(),
            KeyCode::Esc if self.ui.show_discover => self.ui.show_discover = false,
            _ => {}
        }
        Ok(())
    }

    fn handle_terminal_key(&mut self, key: KeyEvent) -> Result<()> {
        // Single host escape hatch: every other key belongs to the remote PTY
        // (Esc, Ctrl-[, Ctrl-W and Ctrl-P are used by vim/readline/tmux).
        let host_escape = matches!(key.code, KeyCode::F(12))
            || (key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('`')));
        if host_escape {
            self.focus_profiles();
        } else {
            self.forward_key(key)?;
        }
        Ok(())
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                match self.sessions.delete_selected() {
                    Ok(()) => self.set_status("profile deleted", false),
                    Err(e) => self.set_status(e.to_string(), true),
                }
                self.ui.mode = Mode::Normal;
                self.ui.focus = Focus::Sessions;
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.ui.mode = Mode::Normal;
                self.ui.focus = Focus::Sessions;
                self.set_status("delete cancelled", false);
            }
            _ => {}
        }
    }

    fn focus_profiles(&mut self) {
        self.ui.focus = Focus::Sessions;
        self.ui.show_discover = false;
        self.set_status(
            "profiles · Enter connect · ]/[ tabs · Ctrl-P palette",
            false,
        );
    }

    fn open_palette(&mut self) {
        self.ui.show_discover = false;
        self.ui.mode = Mode::Palette;
        self.ui.palette = Palette::open_commands();
    }

    fn close_palette(&mut self) {
        self.ui.palette = Palette::default();
        self.ui.mode = Mode::Normal;
    }

    fn apply_locked_theme(&mut self) {
        self.config.theme.name = theme::LOCKED_THEME_ID.to_string();
        theme::set_mode_name(theme::LOCKED_THEME_ID);
        self.set_status("theme locked · AMOLED mono", false);
    }

    fn begin_add(&mut self, ssh: bool) {
        self.ui.show_discover = false;
        self.sessions.begin_add(ssh);
        self.ui.session_form_field = 0;
        self.ui.mode = Mode::SessionForm;
    }

    fn next_ssh_tab(&mut self) {
        if self.sessions.live_ssh.is_empty() {
            self.set_status("no ssh tabs", true);
            return;
        }
        self.sessions.next_ssh_tab();
        self.ui.focus = Focus::Terminal;
    }

    fn prev_ssh_tab(&mut self) {
        if self.sessions.live_ssh.is_empty() {
            self.set_status("no ssh tabs", true);
            return;
        }
        self.sessions.prev_ssh_tab();
        self.ui.focus = Focus::Terminal;
    }

    fn close_ssh_tab(&mut self) {
        if self.sessions.live_ssh.is_empty() {
            self.set_status("no ssh tabs", true);
            return;
        }
        self.sessions.close_active_ssh();
        if self.sessions.live_ssh.is_empty() {
            self.ui.focus = Focus::Sessions;
        }
        self.set_status("ssh tab closed", false);
    }

    fn run_palette_selected(&mut self, term_rows: u16) {
        let Some(id) = self.ui.palette.selected_item().map(|it| it.id.clone()) else {
            self.close_palette();
            return;
        };
        self.close_palette();
        match id.as_str() {
            "connect" => self.begin_session_connect(term_rows),
            "add_ssh" => self.begin_add(true),
            "add_rdp" => self.begin_add(false),
            "edit" => match self.sessions.begin_edit_selected() {
                Ok(()) => {
                    self.ui.session_form_field = 0;
                    self.ui.mode = Mode::SessionForm;
                }
                Err(e) => self.set_status(e.to_string(), true),
            },
            "delete" => {
                if self.sessions.selected_profile().is_some() {
                    self.ui.mode = Mode::ConfirmDelete;
                } else {
                    self.set_status("no profile selected", true);
                }
            }
            "next_tab" => self.next_ssh_tab(),
            "prev_tab" => self.prev_ssh_tab(),
            "close_tab" => self.close_ssh_tab(),
            "clear_password" => self.clear_saved_password(),
            "reload" => match self.sessions.reload() {
                Ok(()) => {
                    self.sessions.reap_rdp();
                    self.set_status(
                        format!("reloaded {} profiles", self.sessions.profiles.len()),
                        false,
                    );
                }
                Err(e) => self.set_status(format!("reload: {e:#}"), true),
            },
            "reload_themes" => {
                theme::reload_customs();
                theme::set_mode_name(theme::LOCKED_THEME_ID);
                self.set_status(
                    format!("themes reloaded · {} custom", theme::customs().len()),
                    false,
                );
            }
            "discover" => {
                self.ui.show_discover = true;
            }
            "quit" => self.should_quit = true,
            _ => {}
        }
    }

    fn clear_saved_password(&mut self) {
        let Some(p) = self.sessions.selected_profile() else {
            self.set_status("no profile selected", true);
            return;
        };
        let id = p.id.clone();
        match ide_sessions::clear_password(&id) {
            Ok(()) => self.set_status("saved SSH password cleared", false),
            Err(e) => self.set_status(format!("clear password: {e:#}"), true),
        }
    }

    fn session_form_field_mut(&mut self) -> &mut String {
        let i = self.ui.session_form_field;
        let ssh = self.sessions.form_kind_ssh;
        match i {
            0 => &mut self.sessions.form_name,
            1 => &mut self.sessions.form_host,
            2 => &mut self.sessions.form_user,
            3 => &mut self.sessions.form_port,
            4 if ssh => &mut self.sessions.form_identity,
            4 => &mut self.sessions.form_gateway,
            _ => &mut self.sessions.form_jump,
        }
    }

    fn begin_session_connect(&mut self, term_rows: u16) {
        let Some(profile) = self.sessions.selected_profile().cloned() else {
            self.set_status("no profile selected", true);
            return;
        };
        match profile.kind {
            SessionKind::Rdp => {
                let cols = 80u16;
                let rows = term_rows.max(6);
                match self.sessions.connect_selected(&self.cwd, cols, rows) {
                    Ok(ConnectOutcome::RdpSpawned { label }) => {
                        self.sessions.clear_failed(&profile.id);
                        self.ui.show_discover = false;
                        self.set_status(format!("rdp launched · {label}"), false);
                    }
                    Ok(ConnectOutcome::SshAttached { .. }) => {}
                    Err(e) => {
                        self.sessions.mark_failed(&profile.id);
                        self.set_status(format!("session: {e}"), true);
                    }
                }
            }
            SessionKind::Ssh => {
                self.ui.show_discover = false;
                self.ui.session_password_profile_id = Some(profile.id.clone());
                self.ui.session_password_input.clear();
                self.ui.session_password_remember = true;
                self.ui.mode = Mode::SessionPassword;
                let hint = if ide_sessions::has_password(&profile.id) {
                    "password · Enter = saved · Tab remember · type to replace"
                } else {
                    "password · Enter connect · Tab remember"
                };
                self.set_status(hint, false);
            }
        }
    }

    fn confirm_session_password(&mut self, term_rows: u16) -> Result<()> {
        let profile_id = self
            .ui
            .session_password_profile_id
            .clone()
            .or_else(|| self.sessions.selected_profile().map(|p| p.id.clone()));
        let Some(profile_id) = profile_id else {
            self.set_status("no profile", true);
            self.ui.mode = Mode::Normal;
            return Ok(());
        };
        let typed = self.ui.session_password_input.clone();
        let remember = self.ui.session_password_remember;
        let password = if typed.is_empty() {
            match ide_sessions::get_password(&profile_id) {
                Some(pw) => Some(pw),
                None => {
                    self.set_status("enter a password", true);
                    return Ok(());
                }
            }
        } else {
            Some(typed)
        };

        // Approximate main pane; the event loop resizes to the exact layout next tick.
        let cols = 100u16;
        let rows = term_rows.max(12);
        match self
            .sessions
            .connect_selected_with_password(&self.cwd, cols, rows, password.clone())
        {
            Ok(ConnectOutcome::SshAttached { label }) => {
                self.sessions.clear_failed(&profile_id);
                self.ui.show_discover = false;
                self.ui.focus = Focus::Terminal;
                self.ui.session_password_input.clear();
                self.ui.mode = Mode::Normal;
                // Give sshpass/ssh a moment; only persist if auth did not fail.
                std::thread::sleep(Duration::from_millis(450));
                let snap = self
                    .sessions
                    .active_ssh_pane()
                    .map(|p| p.snapshot_text().to_ascii_lowercase())
                    .unwrap_or_default();
                let failed = snap.contains("permission denied")
                    || snap.contains("connection refused")
                    || snap.contains("authentication failed");
                if failed {
                    self.sessions.mark_failed(&profile_id);
                    let _ = ide_sessions::clear_password(&profile_id);
                    self.ui.session_password_input.clear();
                    self.ui.session_password_profile_id = Some(profile_id);
                    self.ui.mode = Mode::SessionPassword;
                    self.ui.focus = Focus::Sessions;
                    self.set_status(
                        "auth failed · password is case-sensitive · type again",
                        true,
                    );
                } else if remember {
                    if let Some(pw) = password {
                        match ide_sessions::set_password(&profile_id, &pw) {
                            Ok(()) => {
                                self.set_status(format!("ssh · {label} · password saved"), false)
                            }
                            Err(e) => {
                                self.set_status(format!("ssh · {label} · save failed: {e:#}"), true)
                            }
                        }
                    }
                    self.ui.session_password_profile_id = None;
                } else {
                    self.ui.session_password_profile_id = None;
                    self.set_status(format!("ssh · {label}"), false);
                }
            }
            Ok(ConnectOutcome::RdpSpawned { label }) => {
                self.ui.mode = Mode::Normal;
                self.set_status(format!("rdp launched · {label}"), false);
            }
            Err(e) => {
                self.sessions.mark_failed(&profile_id);
                self.ui.mode = Mode::SessionPassword;
                self.set_status(format!("session: {e}"), true);
            }
        }
        Ok(())
    }

    fn forward_key(&self, key: KeyEvent) -> Result<()> {
        let Some(pane) = self.sessions.active_ssh_pane() else {
            return Ok(());
        };
        self.forward_to_pane(pane, key)
    }

    fn forward_to_pane(&self, pane: &TerminalPane, key: KeyEvent) -> Result<()> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Enter => pane.write_bytes(b"\r")?,
            KeyCode::Backspace => pane.write_bytes(b"\x7f")?,
            KeyCode::Tab => pane.write_bytes(b"\t")?,
            KeyCode::BackTab => pane.write_bytes(b"\x1b[Z")?,
            KeyCode::Esc => pane.write_bytes(b"\x1b")?,
            KeyCode::Up => pane.write_bytes(b"\x1b[A")?,
            KeyCode::Down => pane.write_bytes(b"\x1b[B")?,
            KeyCode::Right => pane.write_bytes(b"\x1b[C")?,
            KeyCode::Left => pane.write_bytes(b"\x1b[D")?,
            KeyCode::Home => pane.write_bytes(b"\x1b[H")?,
            KeyCode::End => pane.write_bytes(b"\x1b[F")?,
            KeyCode::PageUp => pane.write_bytes(b"\x1b[5~")?,
            KeyCode::PageDown => pane.write_bytes(b"\x1b[6~")?,
            KeyCode::Delete => pane.write_bytes(b"\x1b[3~")?,
            KeyCode::Insert => pane.write_bytes(b"\x1b[2~")?,
            KeyCode::F(n) => {
                if let Some(seq) = function_key(n) {
                    pane.write_bytes(seq)?;
                }
            }
            KeyCode::Char(c) => {
                if ctrl {
                    let byte = (c as u32 & 0x1f) as u8;
                    if byte != 0 {
                        pane.write_bytes(&[byte])?;
                    }
                } else if alt {
                    pane.write_bytes(b"\x1b")?;
                    pane.write_char(c)?;
                } else {
                    pane.write_char(c)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn tab_caption(name: &str) -> String {
    let count = name.chars().count();
    if count <= 17 {
        return name.to_string();
    }
    let mut out: String = name.chars().take(16).collect();
    out.push('…');
    out
}

fn function_key(n: u8) -> Option<&'static [u8]> {
    match n {
        1 => Some(b"\x1bOP"),
        2 => Some(b"\x1bOQ"),
        3 => Some(b"\x1bOR"),
        4 => Some(b"\x1bOS"),
        5 => Some(b"\x1b[15~"),
        6 => Some(b"\x1b[17~"),
        7 => Some(b"\x1b[18~"),
        8 => Some(b"\x1b[19~"),
        9 => Some(b"\x1b[20~"),
        10 => Some(b"\x1b[21~"),
        11 => Some(b"\x1b[23~"),
        12 => Some(b"\x1b[24~"),
        _ => None,
    }
}

pub async fn run() -> Result<()> {
    let mut app = App::new()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(33));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        if app.should_quit {
            break;
        }

        let size = terminal.size()?;
        // Header + footer + pane borders ≈ 4 rows; sidebar 34 + borders 2.
        app.expire_status();
        let term_rows = size.height.saturating_sub(4).max(12);
        let cols = size.width.saturating_sub(34).max(40);
        app.sessions.reap_rdp();
        if !app.sessions.live_ssh.is_empty() {
            app.sessions.resize_active_ssh(cols, term_rows);
        }

        let labels: Vec<String> = app
            .sessions
            .ssh_tab_labels()
            .iter()
            .map(|name| tab_caption(name))
            .collect();
        let active = app.sessions.active_ssh;
        let styled = app
            .sessions
            .active_ssh_pane()
            .map(|p| p.styled_rows())
            .unwrap_or_default();
        let lines = app
            .sessions
            .active_ssh_pane()
            .map(|p| p.visible_lines())
            .unwrap_or_default();

        terminal.draw(|frame| {
            render(
                frame,
                RenderCtx {
                    ui: &app.ui,
                    sessions: &app.sessions,
                    terminal_styled: Some(&styled),
                    terminal_lines: Some(&lines),
                    session_tab_labels: &labels,
                    session_tab_active: active,
                },
            );
        })?;

        tokio::select! {
            _ = tick.tick() => {}
            maybe = events.next() => match maybe {
                Some(Ok(event)) => app.handle_event(event, term_rows)?,
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }
    }
    Ok(())
}
