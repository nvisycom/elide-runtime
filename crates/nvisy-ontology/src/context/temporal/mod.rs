//! Date, time, and datetime reference data.

mod date;
mod datetime;
mod time;
mod time_of_day;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::date::DateData;
pub use self::datetime::DateTimeData;
pub use self::time::TimeSpanData;
pub use self::time_of_day::TimeOfDayData;

/// Temporal matching variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TemporalVariant {
    /// Date or date-range to match (e.g. birthdays, expiry dates).
    Date(DateData),
    /// Time-of-day or time range (e.g. business hours).
    TimeOfDay(TimeOfDayData),
    /// Date-time or date-time range (e.g. appointment timestamps).
    DateTime(DateTimeData),
    /// Time span reference for matching audio/video segments.
    TimeSpan(TimeSpanData),
}
