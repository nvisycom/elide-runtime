//! Time span reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitive::TimeSpan;

/// A time span reference for matching audio/video segments.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeSpanData {
    /// The time interval to match.
    pub span: TimeSpan,
    /// Optional human-readable label (e.g. `"intro"`, `"closing remarks"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl TimeSpanData {
    /// Create a time span reference.
    pub fn new(span: TimeSpan) -> Self {
        Self { span, label: None }
    }

    /// Set a label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}
