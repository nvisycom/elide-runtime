//! Date-time reference data.

use jiff::civil::DateTime;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A date-time or date-time range reference for temporal matching.
///
/// Uses naive (timezone-unaware) date-times from [`jiff::civil::DateTime`].
/// For timezone-aware timestamps, use the entry-level `created_at` /
/// `expires_at` fields on [`ContextEntry`](crate::context::ContextEntry).
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
