//! Text redaction output type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Text redaction output — the codec only needs to know the replacement string
/// or that the span should be removed entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextRedactionOutput {
    /// Substituted with a replacement string.
    Replace { replacement: String },
    /// Removed entirely from the output.
    Remove,
}

impl TextRedactionOutput {
    /// Returns the text replacement string, regardless of specific method.
    ///
    /// Returns `None` for [`Remove`](Self::Remove) — the caller should
    /// treat that as an empty string (span deleted).
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Replace { replacement } => Some(replacement),
            Self::Remove => None,
        }
    }
}
