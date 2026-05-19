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

    /// Byte length of the span (`end_offset - start_offset`).
    pub fn len(&self) -> usize {
        self.end_offset.saturating_sub(self.start_offset)
    }

    /// Whether the span is empty (zero length).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Overlap for TextLocation {
    fn overlaps(&self, other: &Self) -> bool {
        self.start_offset < other.end_offset && other.start_offset < self.end_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(start: usize, end: usize) -> TextLocation {
        TextLocation::builder()
            .with_start_offset(start)
            .with_end_offset(end)
            .build()
            .unwrap()
    }

    #[test]
    fn len_and_is_empty() {
        assert_eq!(loc(0, 10).len(), 10);
        assert!(!loc(0, 10).is_empty());
        assert!(loc(5, 5).is_empty());
    }

    #[test]
    fn overlap_intersecting() {
        assert!(loc(0, 10).overlaps(&loc(5, 15)));
    }

    #[test]
    fn overlap_contained() {
        assert!(loc(0, 10).overlaps(&loc(2, 5)));
    }

    #[test]
    fn no_overlap_adjacent() {
        assert!(!loc(0, 5).overlaps(&loc(5, 10)));
    }

    #[test]
    fn no_overlap_disjoint() {
        assert!(!loc(0, 5).overlaps(&loc(10, 15)));
    }

}
