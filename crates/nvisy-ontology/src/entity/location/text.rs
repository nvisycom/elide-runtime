//! Text-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Overlap;

/// Location of an entity within text content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "TextLocationBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct TextLocation {
    /// The matched text at this location. Skipped during serialization
    /// to prevent sensitive data from appearing in API responses.
    #[builder(default)]
    #[serde(skip_serializing)]
    pub value: String,
    /// Byte or character offset where the entity starts.
    pub start_offset: usize,
    /// Byte or character offset where the entity ends.
    pub end_offset: usize,
    /// Start offset of the surrounding context window for redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_start_offset: Option<usize>,
    /// End offset of the surrounding context window for redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_end_offset: Option<usize>,
    /// 1-based page number where the entity was found.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    /// 1-based line number where the entity was found.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

impl TextLocation {
    /// Create a new [`TextLocationBuilder`].
    pub fn builder() -> TextLocationBuilder {
        TextLocationBuilder::default()
    }
}

impl Overlap for TextLocation {
    fn overlaps(&self, other: &Self) -> bool {
        self.start_offset < other.end_offset && other.start_offset < self.end_offset
    }
}
