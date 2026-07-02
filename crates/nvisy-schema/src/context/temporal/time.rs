//! Time-of-day reference data.

use jiff::civil::Time;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A time-of-day or time range reference for temporal matching.
///
/// Uses naive (timezone-unaware) times from [`Time`].
/// Useful for matching recurring time patterns (e.g. business hours).
///
/// [`Time`]: jiff::civil::Time
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeData {
    /// Start (or only) time.
    #[schemars(with = "String")]
    pub start: Time,
    /// End time for a range. When `None` this represents a single time.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub end: Option<Time>,
}

impl TimeData {
    /// Create a single time-of-day reference.
    pub fn single(time: Time) -> Self {
        Self {
            start: time,
            end: None,
        }
    }

    /// Create a time range reference.
    pub fn range(start: Time, end: Time) -> Self {
        Self {
            start,
            end: Some(end),
        }
    }
}
