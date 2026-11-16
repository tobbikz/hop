//! Restrained TUI motion: braille spinner (loading only) + minimal live pulse.
//!
//! Disable with `HOP_ANIMATIONS=0` (or `false` / `off` / `no`).

use std::time::{Duration, Instant};

/// Braille mini-dot frames — same sequence OpenCode uses (≈80 ms).
pub const BRAILLE: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

const SPINNER_MS: u64 = 80;
const PULSE_MS: u64 = 900; // slow, minimal live-dot breathe

#[derive(Debug, Clone, Copy)]
pub struct Anim {
    pub enabled: bool,
    pub elapsed: Duration,
}

impl Anim {
    pub fn new(origin: Instant) -> Self {
        Self {
            enabled: animations_enabled(),
            elapsed: origin.elapsed(),
        }
    }

    pub fn frame(&self, interval_ms: u64, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        if !self.enabled {
            return 0;
        }
        ((self.elapsed.as_millis() as u64) / interval_ms.max(1)) as usize % n
    }

    pub fn braille(&self) -> &'static str {
        if !self.enabled {
            return "⋯";
        }
        BRAILLE[self.frame(SPINNER_MS, BRAILLE.len())]
    }

    /// Phase for live-dot brightness (use `% 2` for faint↔muted).
    pub fn pulse_phase(&self) -> usize {
        self.frame(PULSE_MS, 4)
    }
}

pub fn animations_enabled() -> bool {
    match std::env::var("HOP_ANIMATIONS") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v == "0" || v == "false" || v == "off" || v == "no")
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_cycles_when_enabled() {
        let a = Anim {
            enabled: true,
            elapsed: Duration::from_millis(160),
        };
        assert_eq!(a.braille(), BRAILLE[2]);
        let off = Anim {
            enabled: false,
            elapsed: Duration::from_millis(160),
        };
        assert_eq!(off.braille(), "⋯");
    }
}
