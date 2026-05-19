//! Text redaction instruction types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handler::Mergeable;

/// A text redaction: the *how*. The *where* (byte range within the
/// document) lives on the containing [`TextLocation`] via
/// [`Redactions`]'s `(S, R)` pairs.
///
/// [`TextLocation`]: nvisy_ontology::entity::TextLocation
/// [`Redactions`]: crate::handler::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct TextRedaction {
    /// The redaction output that carries the replacement value.
    pub(crate) output: TextOutput,
}

impl TextRedaction {
    /// Create a new text redaction with the given output.
    pub fn new(output: TextOutput) -> Self {
        Self { output }
    }
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
    /// Create a [`Replace`] output with the given string.
    ///
    /// [`Replace`]: Self::Replace
    pub fn replace(replacement: impl Into<String>) -> Self {
        Self::Replace {
            replacement: replacement.into(),
        }
    }

    /// Returns the text replacement string, regardless of specific method.
    ///
    /// Returns `None` for [`Remove`] — the caller should treat that as
    /// an empty string (span deleted).
    ///
    /// [`Remove`]: Self::Remove
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Replace { replacement } => Some(replacement),
            Self::Remove => None,
        }
    }
}

impl Mergeable for TextRedaction {
    /// Combine two redactions that target overlapping locations. Returns
    /// `Some` only when the outputs match; different replacement
    /// strings cannot be reconciled.
    fn try_merge(self, other: Self) -> Option<Self> {
        (self.output == other.output).then_some(self)
    }
}
