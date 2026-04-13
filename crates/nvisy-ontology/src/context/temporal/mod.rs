//! Date and time-based reference data.

mod date;
mod time;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::date::DateData;
pub use self::time::TimeSpanData;

/// Temporal matching variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TemporalVariant {
    /// Date or date-range to match.
    Date(DateData),
    /// Time span reference for matching audio/video segments.
    TimeSpan(TimeSpanData),
}
