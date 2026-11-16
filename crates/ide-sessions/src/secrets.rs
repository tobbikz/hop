//! Optional SSH passwords (MobaXterm-style), kept out of `sessions.toml`.
//!
//! File: `<config>/sessions-secrets.toml` (mode 0600).
//! Not encrypted at rest — local disk ACL only. Clear anytime from the UI.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
struct SecretsFile {
    #[serde(default)]
    passwords: HashMap<String, String>,
}

fn secrets_path() -> Option<PathBuf> {
    let mut d = ide_core::config_dir()?;
    d.push("sessions-secrets.toml");
    Some(d)
}

fn load_from(path: &Path) -> SecretsFile {
    let Ok(text) = std::fs::read_to_string(path) else {
        return SecretsFile::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

fn save_to(path: &Path, file: &SecretsFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(file)?;
    write_private(path, &text)?;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

fn get_password_at(path: &Path, profile_id: &str) -> Option<String> {
    load_from(path)
        .passwords
        .get(profile_id)
        .cloned()
        .filter(|s| !s.is_empty())
}

fn set_password_at(path: &Path, profile_id: &str, password: &str) -> Result<()> {
    let mut file = load_from(path);
    if password.is_empty() {
        file.passwords.remove(profile_id);
    } else {
        file.passwords
            .insert(profile_id.to_string(), password.to_string());
    }
    save_to(path, &file)
}

pub fn get_password(profile_id: &str) -> Option<String> {
    get_password_at(&secrets_path()?, profile_id)
}

pub fn has_password(profile_id: &str) -> bool {
    get_password(profile_id).is_some()
}

pub fn set_password(profile_id: &str, password: &str) -> Result<()> {
    let path = secrets_path().context("no config dir")?;
    set_password_at(&path, profile_id, password)
}

pub fn clear_password(profile_id: &str) -> Result<()> {
    set_password(profile_id, "")
}

pub fn secrets_file_path() -> Option<PathBuf> {
    secrets_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions-secrets.toml");
        assert_eq!(get_password_at(&path, "p1"), None);
        set_password_at(&path, "p1", "hunter2").unwrap();
        assert_eq!(get_password_at(&path, "p1").as_deref(), Some("hunter2"));
        set_password_at(&path, "p1", "").unwrap();
        assert_eq!(get_password_at(&path, "p1"), None);
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions-secrets.toml");
        set_password_at(&path, "p1", "secret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
