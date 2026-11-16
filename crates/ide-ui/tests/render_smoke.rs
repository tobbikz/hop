//! Layout smoke tests: every overlay renders at common terminal sizes.

use ide_core::{Focus, Mode};
use ide_sessions::{SessionManager, SessionProfile};
use ide_ui::{render, Palette, RenderCtx, StatusMessage, UiState};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn manager() -> SessionManager {
    let mut mgr = SessionManager {
        form_kind_ssh: true,
        form_port: "22".into(),
        form_name: "prod-web".into(),
        ..Default::default()
    };
    mgr.profiles
        .push(SessionProfile::new_ssh("prod-web", "10.0.0.5", "ops"));
    mgr.profiles.push(SessionProfile::new_rdp(
        "win-box",
        "10.0.0.9",
        "Administrator",
    ));
    mgr
}

fn draw(ui: &UiState, sessions: &SessionManager, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render(
                frame,
                RenderCtx {
                    ui,
                    sessions,
                    terminal_styled: None,
                    terminal_lines: None,
                    session_tab_labels: &[],
                    session_tab_active: 0,
                },
            );
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn normal_view_has_header_list_and_hints() {
    let ui = UiState::default();
    let sessions = manager();
    for (w, h) in [(80, 24), (120, 40), (50, 20)] {
        let text = draw(&ui, &sessions, w, h);
        assert!(text.contains("Sessions"), "missing header at {w}x{h}");
        assert!(text.contains("prod-web"), "missing profile at {w}x{h}");
        assert!(text.contains("2 profiles"), "missing count at {w}x{h}");
    }
}

#[test]
fn overlays_render_without_panicking() {
    let sessions = manager();

    let mut ui = UiState {
        show_discover: true,
        ..Default::default()
    };
    assert!(draw(&ui, &sessions, 100, 32).contains("discover"));

    ui.show_discover = false;
    ui.mode = Mode::Palette;
    ui.palette = Palette::open_commands();
    assert!(draw(&ui, &sessions, 100, 32).contains("commands"));

    ui.mode = Mode::SessionForm;
    ui.session_form_field = 3;
    assert!(draw(&ui, &sessions, 100, 32).contains("SSH profile"));

    ui.mode = Mode::SessionPassword;
    ui.session_password_input = "hunter2".into();
    assert!(draw(&ui, &sessions, 100, 32).contains("SSH login password"));

    ui.mode = Mode::ConfirmDelete;
    assert!(draw(&ui, &sessions, 100, 32).contains("Delete profile"));

    ui.mode = Mode::Normal;
    ui.focus = Focus::Terminal;
    ui.status = Some(StatusMessage {
        text: "ssh · prod-web".into(),
        error: false,
    });
    let text = draw(&ui, &sessions, 100, 32);
    assert!(text.contains("ssh · prod-web"));
}
