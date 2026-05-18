//! Time-of-day reference data.

use jiff::civil::Time;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A time-of-day or time range reference for temporal matching.
///
/// Uses naive (timezone-unaware) times from [`jiff::civil::Time`].
/// Useful for matching recurring time patterns (e.g. business hours).
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

#[cfg(test)]
mod tests {
    use jiff::civil::time;

    use super::*;

    #[test]
    fn single_has_no_end() {
        let t = TimeData::single(time(9, 0, 0, 0));
        assert!(t.end.is_none());
    }

    #[test]
    fn range_has_end() {
        let t = TimeData::range(time(9, 0, 0, 0), time(17, 0, 0, 0));
        assert_eq!(t.end, Some(time(17, 0, 0, 0)));
    }

    #[test]
    fn roundtrip_serde() {
        let t = TimeData::range(time(9, 0, 0, 0), time(17, 0, 0, 0));
        let json = serde_json::to_string(&t).unwrap();
        let back: TimeData = serde_json::from_str(&json).unwrap();
        assert_eq!(t.start, back.start);
        assert_eq!(t.end, back.end);
    }
}
