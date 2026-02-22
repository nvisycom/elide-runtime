//! [`ContextRule`] — co-occurrence context for span-level confidence boosting.

use serde::{Deserialize, Serialize};

/// Co-occurrence context rule for span-level confidence boosting.
///
/// When a pattern match is found, nearby spans are searched for any of the
/// `keywords`.  If at least one keyword is present within `window` spans,
/// the match confidence is increased by `boost` (clamped to `[0.0, 1.0]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRule {
    /// Case-insensitive keywords to look for in nearby spans.
    pub keywords: Vec<String>,
    /// Number of spans before and after the match span to search.
    #[serde(default = "default_window")]
    pub window: usize,
    /// Confidence adjustment when at least one keyword is found.
    #[serde(default = "default_boost")]
    pub boost: f64,
}

fn default_window() -> usize {
    3
}

fn default_boost() -> f64 {
    0.1
}
