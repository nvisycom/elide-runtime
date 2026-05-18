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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_serde() {
        let ts = TimeSpanData::new(TimeSpan::from_secs(1.0, 5.5)).with_label("intro");
        let json = serde_json::to_string(&ts).unwrap();
        let back: TimeSpanData = serde_json::from_str(&json).unwrap();
        assert_eq!(ts.span.start_us, back.span.start_us);
        assert_eq!(ts.span.end_us, back.span.end_us);
        assert_eq!(ts.label, back.label);
    }
}
