//! [`PatternDetection`]: pattern-recognizer settings.

use serde::{Deserialize, Serialize};

/// Pattern-recognizer settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternDetection {
    /// Enable this recognizer. When `false`, the recognizer is
    /// neither built nor dispatched, but the config is preserved so
    /// operators can toggle without losing it. Defaults to `true` —
    /// pattern detection is always-on out of the box.
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
