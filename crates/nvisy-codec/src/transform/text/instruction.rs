//! Text redaction instruction types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A located text redaction: pairs a span identifier and intra-span byte
/// range with a [`TextOutput`] that carries the replacement.
pub struct TextRedaction<S> {
    /// Which span this redaction targets.
    pub span_id: S,
    /// Byte offset where the redacted region starts within the span.
    pub start: usize,
    /// Byte offset where the redacted region ends (exclusive) within the span.
    pub end: usize,
    /// The redaction output that carries the replacement value.
    pub output: TextOutput,
}

/// Text redaction output — the codec only needs to know the replacement string
/// or that the span should be removed entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextOutput {
    /// Substituted with a replacement string.
    Replace { replacement: String },
    /// Removed entirely from the output.
    Remove,
}

impl TextOutput {
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
