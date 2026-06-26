//! [`TimeSpanSchema`]: wire shape for [`elide_core::primitive::TimeSpan`].

use elide_core::primitive::TimeSpan;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::primitive::TimeSpan`].
///
/// Microsecond half-open interval `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "TimeSpan")]
pub struct TimeSpanSchema {
    /// Start of the interval, microseconds from the stream start.
    pub start_us: u64,
    /// End of the interval (exclusive), microseconds from the stream
    /// start.
    pub end_us: u64,
}

impl From<TimeSpanSchema> for TimeSpan {
    fn from(s: TimeSpanSchema) -> Self {
        TimeSpan::new(s.start_us, s.end_us)
    }
}

impl From<TimeSpan> for TimeSpanSchema {
    fn from(t: TimeSpan) -> Self {
        Self {
            start_us: t.start_micros(),
            end_us: t.end_micros(),
        }
    }
}
