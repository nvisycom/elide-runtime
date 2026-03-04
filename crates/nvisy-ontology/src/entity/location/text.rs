//! Text-modality entity location.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Location of an entity within text content.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextLocation {
    /// Byte or character offset where the entity starts.
    pub start_offset: usize,
    /// Byte or character offset where the entity ends.
    pub end_offset: usize,
    /// Start offset of the surrounding context window for redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_start_offset: Option<usize>,
    /// End offset of the surrounding context window for redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_end_offset: Option<usize>,
    /// Identifier of the document element containing this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_id: Option<String>,
    /// 1-based page number where the entity was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    /// 1-based line number where the entity was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

impl TextLocation {
    /// Returns `true` if this text location overlaps with `other`.
    pub fn overlaps(&self, other: &TextLocation) -> bool {
        self.start_offset < other.end_offset && other.start_offset < self.end_offset
    }
}
