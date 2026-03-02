//! Date and date-range reference data.

use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A date or date-range reference for temporal matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateData {
    /// Start (or only) date.
    #[schemars(with = "String")]
    pub start: Date,
    /// End date for a range. When `None` this represents a single date.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub end: Option<Date>,
    /// Optional format hint describing the expected representation
    /// (e.g. `"MM/DD/YYYY"`, `"ISO8601"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

impl DateData {
    /// Create a single-date reference.
    pub fn single(date: Date) -> Self {
        Self {
            start: date,
            end: None,
            format: None,
        }
    }

    /// Create a date-range reference.
    pub fn range(start: Date, end: Date) -> Self {
        Self {
            start,
            end: Some(end),
            format: None,
        }
    }

    /// Set a format hint.
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }
}
