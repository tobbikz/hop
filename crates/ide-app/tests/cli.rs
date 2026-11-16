use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tui-sessions"))
}

#[test]
fn version_flag_prints_name_and_version() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("tui-sessions"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_diagnostics_flag() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--doctor"));
    assert!(stdout.to_lowercase().contains("usage"));
}

#[test]
fn doctor_reports_environment() {
    let out = bin().arg("--doctor").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for key in [
        "tui-sessions",
        "os:",
        "config dir:",
        "sessions:",
        "secrets:",
        "ssh:",
        "rdp client:",
    ] {
        assert!(stdout.contains(key), "missing `{key}` in:\n{stdout}");
    }
}

#[test]
fn unknown_flag_fails() {
    let out = bin().arg("--definitely-not-a-flag").output().unwrap();
    assert!(!out.status.success());
}
