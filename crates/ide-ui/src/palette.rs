//! Command palette and session modals (profile form, SSH password).

use crate::theme;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaletteMode {
    #[default]
    Commands,
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub label: String,
    pub id: String,
}

#[derive(Debug, Default)]
pub struct Palette {
    pub mode: PaletteMode,
    pub query: String,
    pub items: Vec<PaletteItem>,
    pub filtered: Vec<usize>,
    pub selected: usize,
}

impl Palette {
    pub fn open_commands() -> Self {
        let items = vec![
            item("Connect selected profile", "connect"),
            item("Add SSH profile", "add_ssh"),
            item("Add RDP profile", "add_rdp"),
            item("Edit selected profile", "edit"),
            item("Delete selected profile", "delete"),
            item("Next SSH tab", "next_tab"),
            item("Previous SSH tab", "prev_tab"),
            item("Close SSH tab", "close_tab"),
            item("Clear saved SSH password", "clear_password"),
            item("Reload sessions from disk", "reload"),
            item("Reload custom themes", "reload_themes"),
            item("Show shortcuts", "discover"),
            item("Quit", "quit"),
        ];
        let mut p = Self {
            mode: PaletteMode::Commands,
            query: String::new(),
            items,
            filtered: Vec::new(),
            selected: 0,
        };
        p.refilter();
        p
    }

    pub fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.items.len()).collect();
        } else {
            let mut matcher = Matcher::new(Config::DEFAULT);
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );
            let mut scored: Vec<(usize, u32)> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, it)| {
                    let mut buf = Vec::new();
                    let hay = Utf32Str::new(&it.label, &mut buf);
                    pattern.score(hay, &mut matcher).map(|score| (i, score))
                })
                .collect();
            scored.sort_by_key(|a| std::cmp::Reverse(a.1));
            self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        }
        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.filtered
            .get(self.selected)
            .and_then(|i| self.items.get(*i))
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.refilter();
    }
}

fn item(label: &str, id: &str) -> PaletteItem {
    PaletteItem {
        label: label.into(),
        id: id.into(),
    }
}

pub fn render_palette(frame: &mut Frame, screen: Rect, palette: &Palette) {
    let width = screen.width.clamp(48, 78);
    let height = screen.height.clamp(10, 20);
    let area = centered(screen, width, height);
    modal_chrome(frame, screen, area);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Plain)
            .border_style(Style::default().fg(theme::border_focus()))
            .style(Style::default().bg(theme::bg()))
            .title(Span::styled(
                " commands ",
                Style::default()
                    .fg(theme::text())
                    .add_modifier(Modifier::BOLD),
            )),
        area,
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // hint
            Constraint::Length(1), // filter
            Constraint::Length(1), // rule
            Constraint::Min(3),    // list
            Constraint::Length(1), // footer
        ])
        .margin(1)
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  type to filter", theme::style_muted()),
            Span::styled(
                format!("  ·  {} matches", palette.filtered.len()),
                theme::style_faint(),
            ),
        ])),
        inner[0],
    );

    let filter_text = if palette.query.is_empty() {
        Line::from(vec![
            Span::styled("  filter  ", theme::style_faint()),
            Span::styled("…", theme::style_faint()),
        ])
    } else {
        Line::from(vec![
            Span::styled("  filter  ", theme::style_faint()),
            Span::styled(
                palette.query.as_str(),
                Style::default()
                    .fg(theme::text())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("▌", theme::style_muted()),
        ])
    };
    frame.render_widget(
        Paragraph::new(filter_text).style(Style::default().bg(theme::bg())),
        inner[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(
                "  {}",
                "─".repeat(inner[2].width.saturating_sub(2) as usize)
            ),
            theme::style_faint(),
        )),
        inner[2],
    );

    let visible = inner[3].height as usize;
    let start = palette.selected.saturating_sub(visible.saturating_sub(1));
    let lines: Vec<Line> = palette
        .filtered
        .iter()
        .skip(start)
        .take(visible)
        .enumerate()
        .map(|(row, idx)| {
            let it = &palette.items[*idx];
            let selected = start + row == palette.selected;
            let (mark, style) = if selected {
                ("▎", theme::style_selected())
            } else {
                (" ", theme::style_text())
            };
            Line::from(vec![
                Span::styled(mark, Style::default().fg(theme::accent())),
                Span::styled(format!(" {} ", it.label), style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner[3]);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "  ↑/↓ move · Enter run · Esc close ",
            theme::style_faint(),
        )),
        inner[4],
    );
}

pub fn render_session_password(
    frame: &mut Frame,
    screen: Rect,
    input: &str,
    profile_label: &str,
    has_saved: bool,
    remember: bool,
) {
    let area = centered(screen, screen.width.clamp(44, 56), 13);
    modal_chrome(frame, screen, area);

    let inner = Block::default().borders(Borders::ALL).inner(area);
    fill_rect(
        frame,
        inner,
        Style::default().bg(theme::bg()).fg(theme::bg()),
    );

    let label = truncate_label(profile_label, (inner.width as usize).saturating_sub(4));
    let masked: String = if input.is_empty() {
        if has_saved {
            "••••••••".into()
        } else {
            "·".repeat(8)
        }
    } else {
        "•".repeat(input.chars().count().clamp(1, 32))
    };
    let field_hint = if input.is_empty() && has_saved {
        "saved · Enter uses it · type to replace"
    } else if input.is_empty() {
        "type password · empty = agent/keys"
    } else {
        "Enter confirm"
    };
    let rule = "─".repeat((inner.width as usize).saturating_sub(4).min(40));

    // Ordered stack: title → target → field → remember → actions.
    let lines = vec![
        Line::from(Span::styled(
            "  SSH login",
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(format!("  {label}"), theme::style_muted())),
        Line::from(""),
        Line::from(Span::styled("  password", theme::style_faint())),
        Line::from(Span::styled(
            format!("  {masked}_"),
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(format!("  {rule}"), theme::style_faint())),
        Line::from(Span::styled(
            format!("  {field_hint}"),
            theme::style_faint(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if remember { "  [x] " } else { "  [ ] " },
                Style::default()
                    .fg(theme::text())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("remember password", theme::style_muted()),
            Span::styled("  Tab", theme::style_faint()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter confirm · Esc cancel",
            theme::style_faint(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_confirm_delete(frame: &mut Frame, screen: Rect, profile_label: &str) {
    let area = centered(screen, screen.width.clamp(44, 56), 10);
    modal_chrome(frame, screen, area);

    let inner = Block::default().borders(Borders::ALL).inner(area);
    fill_rect(
        frame,
        inner,
        Style::default().bg(theme::bg()).fg(theme::bg()),
    );

    let lines = vec![
        Line::from(Span::styled(
            "  Delete profile",
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  “{profile_label}”"),
            theme::style_muted(),
        )),
        Line::from(Span::styled(
            "  Removes it from sessions.toml.",
            theme::style_faint(),
        )),
        Line::from(Span::styled("  Live tabs stay open.", theme::style_faint())),
        Line::from(""),
        Line::from(Span::styled(
            "  Enter/y delete · Esc/n cancel",
            theme::style_muted(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn render_session_form(
    frame: &mut Frame,
    screen: Rect,
    mgr: &ide_sessions::SessionManager,
    field: usize,
) {
    let kind = if mgr.form_kind_ssh { "SSH" } else { "RDP" };
    let edit = if mgr.form_edit_id.is_some() {
        "edit"
    } else {
        "add"
    };
    let fields = [
        ("name", mgr.form_name.as_str()),
        ("host", mgr.form_host.as_str()),
        ("user", mgr.form_user.as_str()),
        ("port", mgr.form_port.as_str()),
        (
            if mgr.form_kind_ssh {
                "identity_file"
            } else {
                "gateway"
            },
            if mgr.form_kind_ssh {
                mgr.form_identity.as_str()
            } else {
                mgr.form_gateway.as_str()
            },
        ),
        (
            if mgr.form_kind_ssh {
                "jump_host"
            } else {
                "(unused)"
            },
            mgr.form_jump.as_str(),
        ),
    ];

    let width = screen.width.clamp(40, 64);
    let height = screen.height.clamp(14, 18);
    let area = centered(screen, width, height);
    modal_chrome(frame, screen, area);

    let inner = Block::default().borders(Borders::ALL).inner(area);
    fill_rect(
        frame,
        inner,
        Style::default().bg(theme::bg()).fg(theme::bg()),
    );

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            format!("  {edit} {kind} profile"),
            Style::default()
                .fg(theme::text())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (i, (label, val)) in fields.iter().enumerate() {
        let selected = i == field;
        let prefix = if selected {
            Span::styled("▎", Style::default().fg(theme::accent()))
        } else {
            Span::styled(" ", theme::style_faint())
        };
        let label_s = if selected {
            Style::default()
                .fg(theme::accent())
                .add_modifier(Modifier::BOLD)
        } else {
            theme::style_muted()
        };
        let val_s = if selected {
            theme::style_selected()
        } else {
            theme::style_text()
        };
        lines.push(Line::from(vec![
            prefix,
            Span::styled(format!(" {label:<14} "), label_s),
            Span::styled(format!(" {val} "), val_s),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  no passwords stored — ssh-agent / -i key only",
        theme::style_faint(),
    )));
    lines.push(Line::from(Span::styled(
        "  ↓/Tab next · ↑/S-Tab prev · Enter save · Esc",
        theme::style_faint(),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn centered(screen: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(screen.width);
    let height = height.min(screen.height);
    Rect {
        x: screen.x + (screen.width.saturating_sub(width)) / 2,
        y: screen.y + (screen.height.saturating_sub(height)) / 3,
        width,
        height,
    }
}

/// Opaque modal stack: Clear + cell fill (Block bg alone does not erase glyphs).
fn modal_chrome(frame: &mut Frame, screen: Rect, area: Rect) {
    frame.render_widget(Clear, screen);
    fill_rect(
        frame,
        screen,
        Style::default().bg(theme::scrim()).fg(theme::scrim()),
    );
    fill_rect(
        frame,
        area,
        Style::default().bg(theme::bg()).fg(theme::bg()),
    );
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Plain)
            .border_style(Style::default().fg(theme::border_focus()))
            .style(Style::default().bg(theme::bg())),
        area,
    );
}

fn fill_rect(frame: &mut Frame, area: Rect, style: Style) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let row = " ".repeat(area.width as usize);
    let lines: Vec<Line> = (0..area.height).map(|_| Line::from(row.as_str())).collect();
    frame.render_widget(Paragraph::new(lines).style(style), area);
}

fn truncate_label(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".into();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}
