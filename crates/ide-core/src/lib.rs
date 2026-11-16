//! Shared types and helpers for tui-sessions.

use std::path::PathBuf;

/// Input mode of the main screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    /// Fuzzy command palette overlay.
    Palette,
    /// Add / edit session profile form.
    SessionForm,
    /// SSH password prompt.
    SessionPassword,
}

/// Which pane owns keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Sessions,
    Terminal,
}

/// Resolve the config directory, preferring `tui-sessions` over the legacy
/// `tui-ide` name when only the latter exists.
pub fn config_dir() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let current = base.join("tui-sessions");
    let legacy = base.join("tui-ide");
    Some(if current.exists() || !legacy.exists() {
        current
    } else {
        legacy
    })
}
