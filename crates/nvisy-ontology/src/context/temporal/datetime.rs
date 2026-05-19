//! Date-time reference data.

use jiff::civil::DateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A date-time or date-time range reference for temporal matching.
///
/// Uses naive (timezone-unaware) date-times from [`jiff::civil::DateTime`].
/// For timezone-aware timestamps, use the entry-level `created_at` /
/// `expires_at` fields on [`ContextEntry`].
///
/// [`ContextEntry`]: crate::context::ContextEntry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeData {
    /// Start (or only) date-time.
    #[schemars(with = "String")]
    pub start: DateTime,
    /// End date-time for a range. When `None` this represents a single instant.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub end: Option<DateTime>,
}

impl DateTimeData {
    /// Create a single date-time reference.
    pub fn single(dt: DateTime) -> Self {
        Self {
            start: dt,
            end: None,
        }
    }

    /// Create a date-time range reference.
    pub fn range(start: DateTime, end: DateTime) -> Self {
        Self {
            start,
            end: Some(end),
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::datetime;

    use super::*;

    #[test]
    fn roundtrip_serde() {
        let dt = DateTimeData::range(
            datetime(2026, 1, 1, 9, 0, 0, 0),
            datetime(2026, 1, 1, 17, 0, 0, 0),
        );
        let json = serde_json::to_string(&dt).unwrap();
        let back: DateTimeData = serde_json::from_str(&json).unwrap();
        assert_eq!(dt.start, back.start);
        assert_eq!(dt.end, back.end);
    }

    #[test]
    fn single_skips_end_in_json() {
        let dt = DateTimeData::single(datetime(2026, 1, 1, 9, 0, 0, 0));
        let json = serde_json::to_string(&dt).unwrap();
        assert!(!json.contains("\"end\""), "end should be skipped: {json}");
    }
}
