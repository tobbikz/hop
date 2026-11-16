//! Session list, live PTY pane, overlays and command palette.

mod palette;
pub mod theme;

pub use palette::{Palette, PaletteItem, PaletteMode};

use ide_core::{Focus, Mode};
use ide_sessions::SessionManager;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

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
        .constraints([Constraint::Length(32), Constraint::Min(24)])
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
        Mode::ConfirmDelete => {
            let name = ctx
                .sessions
                .selected_profile()
                .map(|p| p.name.clone())
                .unwrap_or_default();
            palette::render_confirm_delete(frame, size, &name);
        }
        Mode::Normal => {}
    }
}

fn render_header(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let n = ctx.sessions.profiles.len();
    let live = ctx.sessions.live_ssh.len() + ctx.sessions.live_rdp.len();
    let profile_word = if n == 1 { "profile" } else { "profiles" };
    let state = if live == 0 {
        "idle".to_string()
    } else {
        format!("{live} connected")
    };

    let mut spans = vec![
        Span::styled(
            " Sessions ",
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {n} {profile_word} "), theme::style_muted()),
        Span::styled("· ", theme::style_faint()),
        Span::styled(
            format!("{state} "),
            if live > 0 {
                theme::style_accent()
            } else {
                theme::style_faint()
            },
        ),
    ];

    let left_w: usize = spans.iter().map(|s| s.content.width()).sum();
    let theme_label = format!("{} ", theme::current_label());
    let right_w = theme_label.width();
    let pad = (area.width as usize).saturating_sub(left_w + right_w);
    if pad >= 2 {
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(theme_label, theme::style_faint()));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::surface())),
        area,
    );
}

fn render_footer(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let line = match &ctx.ui.status {
        Some(status) => {
            let (icon, style) = if status.error {
                (" ! ", theme::style_err())
            } else {
                (" ✓ ", theme::style_ok())
            };
            Line::from(vec![
                Span::styled(icon, style),
                Span::styled(status.text.clone(), theme::style_text()),
            ])
        }
        None if ctx.ui.show_discover => Line::from(Span::styled(
            " Space / Esc close discover ",
            theme::style_faint(),
        )),
        None if ctx.ui.focus == Focus::Terminal => Line::from(Span::styled(
            " typing in SSH · Ctrl-` / F12 back to profiles · tabs from the list ",
            theme::style_faint(),
        )),
        None => Line::from(Span::styled(
            " Enter connect · a/A add · e edit · x delete · ]/[ tabs · Space keys · Ctrl-P ",
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
    let inner_w = area.width.saturating_sub(2) as usize;
    let mut items: Vec<ListItem> = Vec::new();

    if sessions.profiles.is_empty() {
        items.push(ListItem::new(Line::from("")));
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
        for (i, p) in sessions.profiles.iter().enumerate() {
            let selected = i == sessions.selected;
            let live = sessions.live_ssh.iter().any(|l| l.profile_id == p.id)
                || sessions.live_rdp.iter().any(|l| l.profile_id == p.id);
            let failed = sessions.failed.contains(&p.id);
            let state = if live {
                "●"
            } else if failed {
                "○"
            } else {
                " "
            };

            let target = if p.user.is_empty() {
                p.host.clone()
            } else {
                format!("{}@{}", p.user, p.host)
            };
            let target = if p.kind == ide_sessions::SessionKind::Rdp {
                format!("rdp · {target}")
            } else {
                target
            };

            // prefix + space + name + pad + target + gap + dot
            let fixed = 1 + 1 + 2 + 1;
            let avail = inner_w.saturating_sub(fixed);
            let name = truncate(&p.name, (avail * 45 / 100).max(4));
            let name_w = name.width();
            let target = truncate(&target, avail.saturating_sub(name_w));
            let used = 1 + 1 + name_w + target.width() + 2 + 1;
            let pad = inner_w.saturating_sub(used);

            let row_bg = if selected {
                if focused {
                    theme::surface_3()
                } else {
                    theme::surface_2()
                }
            } else {
                theme::surface()
            };
            let bar_style =
                Style::default()
                    .bg(row_bg)
                    .fg(if selected { theme::accent() } else { row_bg });
            let name_style = Style::default()
                .bg(row_bg)
                .fg(if selected {
                    theme::text()
                } else {
                    theme::muted()
                })
                .add_modifier(if selected && focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            let target_style = Style::default().bg(row_bg).fg(theme::faint());
            let dot_style = Style::default().bg(row_bg).fg(if live {
                theme::accent()
            } else if failed {
                theme::danger()
            } else {
                row_bg
            });

            let mut spans = vec![
                Span::styled(if selected { "▎" } else { " " }, bar_style),
                Span::styled(" ", Style::default().bg(row_bg)),
                Span::styled(name, name_style),
            ];
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), Style::default().bg(row_bg)));
            }
            spans.push(Span::styled(target, target_style));
            spans.push(Span::styled("  ", Style::default().bg(row_bg)));
            spans.push(Span::styled(state, dot_style));
            items.push(ListItem::new(Line::from(spans)));
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

fn terminal_title(ctx: &RenderCtx<'_>, focused: bool) -> Line<'static> {
    if ctx.session_tab_labels.is_empty() {
        return Line::from(Span::styled(
            " Session · Enter on a profile to connect ",
            theme::style_title(focused),
        ));
    }
    let mut spans = vec![Span::styled(" SSH ", theme::style_faint())];
    for (i, label) in ctx.session_tab_labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("│", theme::style_faint()));
        }
        if i == ctx.session_tab_active {
            let style = if focused {
                theme::style_pill_on(theme::accent())
            } else {
                Style::default()
                    .fg(theme::text())
                    .bg(theme::surface_3())
                    .add_modifier(Modifier::BOLD)
            };
            spans.push(Span::styled(format!(" {label} "), style));
        } else {
            spans.push(Span::styled(format!(" {label} "), theme::style_muted()));
        }
    }
    spans.push(Span::styled("  Ctrl-` profiles ", theme::style_faint()));
    Line::from(spans)
}

fn render_terminal(frame: &mut Frame, area: Rect, ctx: &RenderCtx<'_>) {
    let focused = ctx.ui.focus == Focus::Terminal;
    let border = theme::border(focused);
    let title = terminal_title(ctx, focused);

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
                    .title(title)
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
                .title(title)
                .border_style(Style::default().fg(border))
                .padding(Padding::new(1, 1, 0, 0))
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
            ("Ctrl-` / F12", "back to profiles"),
            ("all keys", "go to the SSH session"),
        ]
    } else {
        &[
            ("Enter", "connect selected"),
            ("a  A", "add SSH · add RDP"),
            ("e  x", "edit · delete"),
            ("]  [", "next / prev SSH tab"),
            ("Ctrl-W", "close SSH tab"),
            ("Ctrl-P", "command palette"),
            ("Ctrl-Q", "quit"),
        ]
    };
    let scope = if ui.focus == Focus::Terminal {
        "Session"
    } else {
        "Profiles"
    };

    let width = screen.width.clamp(34, 46);
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

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.width() <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".into();
    }
    let mut out = String::new();
    for ch in s.chars() {
        let next = out.width() + ch.to_string().width();
        if next > max - 1 {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}
