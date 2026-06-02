//! [`PatternDetection`]: pattern-recognizer settings.
//!
//! Today the only knob is the enable/disable toggle — the
//! registry composition (which shipped patterns + dictionaries
//! ship, which extras to load) isn't yet plan-configurable.

use serde::{Deserialize, Serialize};

/// Pattern-recognizer settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true` — pattern detection is always-on out of the box.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for PatternDetection {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_true() -> bool {
    true
}
