//! Shared types and helpers for Hop.

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
    /// Delete-profile confirmation.
    ConfirmDelete,
}

/// Which pane owns keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Sessions,
    Terminal,
}

/// Resolve the config directory.
/// Preference: `hop` → `tui-sessions` → `tui-ide` (first that exists, else create `hop`).
pub fn config_dir() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let hop = base.join("hop");
    let sessions = base.join("tui-sessions");
    let ide = base.join("tui-ide");
    Some(if hop.exists() {
        hop
    } else if sessions.exists() {
        sessions
    } else if ide.exists() {
        ide
    } else {
        hop
    })
}

/// Product display name.
pub const PRODUCT_NAME: &str = "Hop";

/// CLI / binary name.
pub const PRODUCT_BIN: &str = "hop";
