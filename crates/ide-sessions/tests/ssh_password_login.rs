//! Manual-style integration: password login via sshpass path (localhost).
//! Skips unless TUI_SESSIONS_SSH_PASS and sshd/sshpass are available.

use ide_sessions::SessionManager;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn localhost_password_login_via_sshpass() {
    let pass = match std::env::var("TUI_SESSIONS_SSH_PASS") {
        Ok(p) if !p.is_empty() => p,
        _ => {
            eprintln!("skip: set TUI_SESSIONS_SSH_PASS to run");
            return;
        }
    };
    if std::process::Command::new("sshpass")
        .arg("-V")
        .output()
        .is_err()
    {
        eprintln!("skip: sshpass missing");
        return;
    }

    let user = std::env::var("TUI_SESSIONS_SSH_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "iks".into());
    let host = std::env::var("TUI_SESSIONS_SSH_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("TUI_SESSIONS_SSH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(22);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.toml");
    let mut mgr = SessionManager {
        path: Some(path),
        form_kind_ssh: true,
        form_port: port.to_string(),
        ..Default::default()
    };
    let mut p = ide_sessions::SessionProfile::new_ssh("local", &host, &user);
    p.port = port;
    p.identity_file.clear();
    mgr.profiles.push(p);
    mgr.selected = 0;

    let cwd = PathBuf::from("/tmp");
    let out = mgr
        .connect_selected_with_password(&cwd, 80, 24, Some(pass))
        .expect("connect");
    assert!(matches!(
        out,
        ide_sessions::ConnectOutcome::SshAttached { .. }
    ));

    // Wait for auth / fancy shell MOTD (neofetch) to finish.
    let mut ready = false;
    let mut last = String::new();
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(150));
        last = mgr
            .active_ssh_pane()
            .map(|p| p.snapshot_text())
            .unwrap_or_default();
        let low = last.to_ascii_lowercase();
        assert!(!low.contains("permission denied"), "auth failed: {last}");
        if last.contains('›') || last.contains("$ ") || last.contains("% ") || last.contains(&user)
        {
            ready = true;
            break;
        }
    }
    assert!(ready, "shell never became ready: {last:?}");

    {
        let pane = mgr.active_ssh_pane().expect("pane");
        pane.write_bytes(b"echo TUI_SESSIONS_OK\n").unwrap();
    }
    let mut ok = false;
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(150));
        let snap = mgr
            .active_ssh_pane()
            .map(|p| p.snapshot_text())
            .unwrap_or_default();
        if snap.contains("TUI_SESSIONS_OK") {
            ok = true;
            eprintln!("PTY ok:\n{snap}");
            break;
        }
    }
    assert!(ok, "shell not interactive / echo missing");
}
