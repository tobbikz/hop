//! Session list, live PTY pane, overlays and command palette.

mod palette;
pub mod theme;

pub use palette::{Palette, PaletteItem, PaletteMode};

use ide_core::{Focus, Mode};
use ide_sessions::SessionManager;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub error: bool,
}

pub struct UiState {
    pub mode: Mode,
    pub focus: Focus,
    pub palette: Palette,
    pub status: Option<StatusMessage>,
    /// Space discoverability overlay.
    pub show_discover: bool,
    /// SessionForm field index: 0 name … 5 jump/gateway.
    pub session_form_field: usize,
    /// SSH password modal input (masked).
    pub session_password_input: String,
    /// Profile id waiting for password.
    pub session_password_profile_id: Option<String>,
    /// On password modal: remember after successful connect.
    pub session_password_remember: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            focus: Focus::Sessions,
            palette: Palette::default(),
            status: None,
            show_discover: false,
            session_form_field: 0,
            session_password_input: String::new(),
            session_password_profile_id: None,
            session_password_remember: true,
        }
    }
}

pub struct RenderCtx<'a> {
    pub ui: &'a UiState,
    pub sessions: &'a SessionManager,
    /// VT100 styled rows for the active SSH PTY (preferred over `terminal_lines`).
    pub terminal_styled: Option<&'a [Vec<ide_term::TermSpan>]>,
    pub terminal_lines: Option<&'a [String]>,
    pub session_tab_labels: &'a [String],
    pub session_tab_active: usize,
}

pub fn render(frame: &mut Frame, ctx: RenderCtx<'_>) {
    let size = frame.area();
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(size);

    render_header(frame, root[0], &ctx);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(24)])
        .split(root[1]);
    frame.render_widget(
        Block::default().style(Style::default().bg(theme::bg())),
        cols[0],
    );
    render_profiles(
        frame,
        cols[0],
        ctx.sessions,
        ctx.ui.focus == Focus::Sessions,
    );
    render_terminal(frame, cols[1], &ctx);
    render_footer(frame, root[2], &ctx);

    if ctx.ui.show_discover {
        render_discover(frame, size, ctx.ui);
    }

    match ctx.ui.mode {
        Mode::Palette => palette::render_palette(frame, size, &ctx.ui.palette),
        Mode::SessionForm => {
            palette::render_session_form(frame, size, ctx.sessions, ctx.ui.session_form_field)
        }
        Mode::SessionPassword => {
            let id = ctx.ui.session_password_profile_id.as_deref();
            let label = ctx
                .sessions
                .profiles
                .iter()
                .find(|p| Some(p.id.as_str()) == id)
                .map(|p| p.display_label())
                .or_else(|| ctx.sessions.selected_profile().map(|p| p.display_label()))
                .unwrap_or_else(|| "SSH".into());
            let saved = id.map(ide_sessions::has_password).unwrap_or_else(|| {
                ctx.sessions
                    .selected_profile()
                    .map(|p| ide_sessions::has_password(&p.id))
                    .unwrap_or(false)
            });
            palette::render_session_password(
                frame,
                size,
                &ctx.ui.session_password_input,
                &label,
                saved,
                ctx.ui.session_password_remember,
            );
        }
        Mode::Normal => {}
    }
}

fn render_header(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let n = ctx.sessions.profiles.len();
    let live = ctx.sessions.live_ssh.len() + ctx.sessions.live_rdp.len();
    let line = Line::from(vec![
        Span::styled(" Sessions ", theme::style_selected_strong()),
        Span::styled(format!(" {n} profiles "), theme::style_muted()),
        if live > 0 {
            Span::styled(format!(" · {live} live "), theme::style_accent())
        } else {
            Span::styled(" · idle ", theme::style_faint())
        },
        Span::styled(
            format!(" · {} ", theme::current_label()),
            theme::style_faint(),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::surface())),
        area,
    );
}

fn render_footer(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let line = match &ctx.ui.status {
        Some(status) => {
            let style = if status.error {
                theme::style_err()
            } else {
                theme::style_muted()
            };
            Line::from(vec![
                Span::styled(if status.error { " ! " } else { " · " }, style),
                Span::styled(status.text.clone(), style),
            ])
        }
        None if ctx.ui.show_discover => Line::from(Span::styled(
            " Space / Esc close discover ",
            theme::style_faint(),
        )),
        None if ctx.ui.focus == Focus::Terminal => Line::from(Span::styled(
            " typing in SSH  ·  Esc profiles  ·  Ctrl-] next  ·  Ctrl-W close ",
            theme::style_faint(),
        )),
        None => Line::from(Span::styled(
            " Enter connect  ·  a/A add  ·  e edit  ·  x del  ·  Space discover  ·  Ctrl-P ",
            theme::style_faint(),
        )),
    };
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::surface())),
        area,
    );
}

fn render_profiles(frame: &mut Frame, area: Rect, sessions: &SessionManager, focused: bool) {
    let border = theme::border(focused);
    let mut items: Vec<ListItem> = Vec::new();
    if sessions.profiles.is_empty() {
        items.push(ListItem::new(Span::styled(
            " No profiles yet",
            theme::style_muted(),
        )));
        items.push(ListItem::new(Span::styled(
            " a add SSH · A add RDP",
            theme::style_faint(),
        )));
        items.push(ListItem::new(Span::styled(
            " ~/.config/tui-sessions/sessions.toml",
            theme::style_faint(),
        )));
    } else {
        items.push(ListItem::new(Span::styled(
            " Enter · a SSH · A RDP · Space ",
            theme::style_faint(),
        )));
        for (i, p) in sessions.profiles.iter().enumerate() {
            let selected = i == sessions.selected;
            let live = sessions.live_ssh.iter().any(|l| l.profile_id == p.id)
                || sessions.live_rdp.iter().any(|l| l.profile_id == p.id);
            let mark = if live { "▸" } else { " " };
            let kind = match p.kind {
                ide_sessions::SessionKind::Ssh => "ssh",
                ide_sessions::SessionKind::Rdp => "rdp",
            };
            let target = if p.user.is_empty() {
                p.host.clone()
            } else {
                format!("{}@{}", p.user, p.host)
            };
            let row = format!("{mark} {}  {kind}  {target}", p.name);
            let style = if selected {
                theme::style_selected()
            } else {
                theme::style_text()
            };
            let prefix = if selected {
                Span::styled("▎", Style::default().fg(theme::accent()))
            } else {
                Span::styled(" ", theme::style_faint())
            };
            items.push(ListItem::new(Line::from(vec![
                prefix,
                Span::styled(row, style),
            ])));
        }
        if !sessions.live_rdp.is_empty() {
            items.push(ListItem::new(Span::styled(
                format!(" ── rdp · {} ── ", sessions.live_rdp.len()),
                theme::style_faint(),
            )));
        }
    }
    frame.render_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(Span::styled(" Profiles ", theme::style_title(focused)))
                    .border_style(Style::default().fg(border))
                    .style(Style::default().bg(theme::surface())),
            )
            .style(Style::default().bg(theme::surface())),
        area,
    );
}

fn render_terminal(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let focused = ctx.ui.focus == Focus::Terminal;
    let border = theme::border(focused);
    let title = if ctx.session_tab_labels.is_empty() {
        " Session · Enter on a profile to connect ".to_string()
    } else {
        let tabs: String = ctx
            .session_tab_labels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == ctx.session_tab_active {
                    format!("[{l}]")
                } else {
                    format!(" {l} ")
                }
            })
            .collect::<Vec<_>>()
            .join("│");
        format!(" SSH {tabs} · Ctrl-] next · Ctrl-W close ")
    };

    let styled = ctx.terminal_styled.unwrap_or(&[]);
    let lines = ctx.terminal_lines.unwrap_or(&[]);
    if styled.is_empty() && lines.is_empty() {
        let hint = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Connect a profile from the left (Enter).",
                theme::style_muted(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  SSH opens a PTY here · RDP launches an external client.",
                theme::style_faint(),
            )),
            Line::from(Span::styled(
                "  No passwords in TOML — ssh-agent / identity file only.",
                theme::style_faint(),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  Space",
                    Style::default()
                        .fg(theme::accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" discover · ", theme::style_faint()),
                Span::styled(
                    "Ctrl-P",
                    Style::default()
                        .fg(theme::accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" palette · ", theme::style_faint()),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(hint).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(Span::styled(title, theme::style_title(focused)))
                    .border_style(Style::default().fg(border))
                    .style(Style::default().bg(theme::bg())),
            ),
            area,
        );
        return;
    }

    let text: Vec<Line> = if !styled.is_empty() {
        styled_rows_to_lines(styled)
    } else {
        lines
            .iter()
            .map(|l| Line::from(Span::styled(l.clone(), theme::style_text())))
            .collect()
    };
    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(title, theme::style_title(focused)))
                .border_style(Style::default().fg(border))
                .style(Style::default().bg(theme::bg()).fg(theme::text())),
        ),
        area,
    );
}

fn styled_rows_to_lines(rows: &[Vec<ide_term::TermSpan>]) -> Vec<Line<'static>> {
    rows.iter()
        .map(|row| {
            if row.is_empty() {
                return Line::from(Span::raw(""));
            }
            let spans: Vec<Span> = row
                .iter()
                .map(|s| {
                    let bg = s.bg.unwrap_or_else(theme_bg_rgb);
                    let fg = contrast_terminal_fg(s.fg, bg);
                    let mut style = Style::default().fg(Color::Rgb(fg.0, fg.1, fg.2));
                    if s.bg.is_some() {
                        style = style.bg(Color::Rgb(bg.0, bg.1, bg.2));
                    }
                    if s.bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    Span::styled(s.text.clone(), style)
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

fn theme_bg_rgb() -> (u8, u8, u8) {
    match theme::bg() {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

fn theme_text_rgb() -> (u8, u8, u8) {
    match theme::text() {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (230, 230, 230),
    }
}

fn luminance(r: u8, g: u8, b: u8) -> i16 {
    // Rec. 601 luma, scaled 0..255.
    ((r as i16 * 30) + (g as i16 * 59) + (b as i16 * 11)) / 100
}

/// Keep PTY glyphs readable against the pane.
fn contrast_terminal_fg(fg: Option<(u8, u8, u8)>, bg: (u8, u8, u8)) -> (u8, u8, u8) {
    let fallback = theme_text_rgb();
    let Some((r, g, b)) = fg else {
        return fallback;
    };
    if (luminance(r, g, b) - luminance(bg.0, bg.1, bg.2)).abs() < 70 {
        return fallback;
    }
    (r, g, b)
}

fn render_discover(frame: &mut Frame, screen: Rect, ui: &UiState) {
    frame.render_widget(
        Block::default().style(
            Style::default()
                .bg(theme::scrim())
                .add_modifier(Modifier::DIM),
        ),
        screen,
    );

    let entries: &[(&str, &str)] = if ui.focus == Focus::Terminal {
        &[
            ("Ctrl-]", "next tab"),
            ("Ctrl-[", "prev tab"),
            ("Ctrl-W", "close tab"),
            ("Ctrl-P", "command palette"),
            ("Esc", "back to profiles"),
        ]
    } else {
        &[
            ("Enter", "connect selected"),
            ("a  A", "add SSH · add RDP"),
            ("e  x", "edit · delete"),
            ("Ctrl-]", "next SSH tab"),
            ("Ctrl-W", "close SSH tab"),
            ("Ctrl-P", "command palette"),
        ]
    };
    let scope = if ui.focus == Focus::Terminal {
        "Session"
    } else {
        "Profiles"
    };

    let width = screen.width.clamp(34, 44);
    let height = (entries.len() as u16)
        .saturating_add(5)
        .min(screen.height.saturating_sub(4))
        .max(8);
    let area = Rect {
        x: screen.x + (screen.width.saturating_sub(width)) / 2,
        y: screen.y + (screen.height.saturating_sub(height)) / 3,
        width,
        height,
    };
    frame.render_widget(ratatui::widgets::Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::accent()))
        .style(Style::default().bg(theme::modal()))
        .title(Span::styled(
            format!(" discover · {scope} "),
            Style::default()
                .fg(theme::accent())
                .add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(Span::styled(
            " space / esc ",
            theme::style_faint(),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height >= 2 {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "─".repeat(inner.width as usize),
                Style::default().fg(theme::border_idle()),
            )),
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
        );
    }

    let body = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };
    let lines: Vec<Line> = entries
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled("  ", theme::style_faint()),
                Span::styled(
                    format!("{k:<9}"),
                    Style::default()
                        .fg(theme::accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", theme::style_faint()),
                Span::styled((*d).to_string(), theme::style_muted()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), body);
}
