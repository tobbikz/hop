//! RDP launch path: a fake `xfreerdp` on PATH must be spawned and reaped.

#![cfg(unix)]

use ide_sessions::{ConnectOutcome, SessionManager, SessionProfile};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
fn rdp_spawns_external_client() {
    let bin_dir = tempfile::tempdir().unwrap();
    let client = bin_dir.path().join("xfreerdp");
    std::fs::write(&client, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o755)).unwrap();

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.path().to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    let new_path = std::env::join_paths(paths).unwrap();
    std::env::set_var("PATH", &new_path);

    let dir = tempfile::tempdir().unwrap();
    let mut mgr = SessionManager {
        path: Some(dir.path().join("sessions.toml")),
        form_kind_ssh: false,
        form_port: "3389".into(),
        ..Default::default()
    };
    let mut p = SessionProfile::new_rdp("lab", "127.0.0.1", "Administrator");
    p.port = 3389;
    mgr.profiles.push(p);
    mgr.selected = 0;

    let outcome = mgr
        .connect_selected(std::path::Path::new("/tmp"), 80, 24)
        .expect("rdp connect");
    std::env::set_var("PATH", old_path);

    assert!(matches!(outcome, ConnectOutcome::RdpSpawned { .. }));
    assert_eq!(mgr.live_rdp.len(), 1);

    for _ in 0..100 {
        mgr.reap_rdp();
        if mgr.live_rdp.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(mgr.live_rdp.is_empty(), "rdp child was not reaped");
}
