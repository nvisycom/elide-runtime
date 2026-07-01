//! Data retention duration types.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How long data is retained.
///
/// Ordered from strictest to laxest — `ZeroRetention <
/// Duration { days: N } < Indefinite`, and within `Duration`,
/// smaller `days` is stricter (`Duration { days: 7 } <
/// Duration { days: 30 }`). The derived [`Ord`] reflects this:
/// strictest-wins resolution across multiple policies is just
/// `iter.min()`. Variant declaration order is load-bearing —
/// don't reorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Retention {
    /// Delete data immediately after processing.
    ZeroRetention,
    /// Retain data for a fixed number of days.
    Duration {
        /// Maximum number of days to retain data.
        days: u64,
    },
    /// Retain data indefinitely.
    Indefinite,
}

impl Retention {
    /// Returns the retention duration.
    ///
    /// Returns [`Duration::ZERO`] for `ZeroRetention` and `None` for `Indefinite`.
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::ZeroRetention => Some(Duration::ZERO),
            Self::Duration { days } => Some(Duration::from_secs(days * 24 * 60 * 60)),
            Self::Indefinite => None,
        }
    }
}
