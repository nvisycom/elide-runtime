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
    /// The matched text at this location.
    ///
    /// Populated during detection; skipped in serialization to prevent
    /// sensitive data from appearing in API responses.
    #[builder(default)]
    #[serde(default, skip_serializing)]
    pub text: String,
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
    fn builder_required_fields() {
        let loc = TextLocation::builder()
            .with_start_offset(0usize)
            .with_end_offset(5usize)
            .build()
            .unwrap();
        assert_eq!(loc.start_offset, 0);
        assert_eq!(loc.end_offset, 5);
        assert!(loc.text.is_empty());
        assert_eq!(loc.line_number, None);
    }

    #[test]
    fn builder_with_text() {
        let loc = TextLocation::builder()
            .with_text("hello")
            .with_start_offset(0usize)
            .with_end_offset(5usize)
            .build()
            .unwrap();
        assert_eq!(loc.text, "hello");
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

    #[test]
    fn serde_round_trip() {
        let loc = TextLocation::builder()
            .with_text("secret")
            .with_start_offset(0usize)
            .with_end_offset(6usize)
            .build()
            .unwrap();
        let json = serde_json::to_string(&loc).unwrap();
        // text field is skip_serializing
        assert!(!json.contains("secret"));
        let deser: TextLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.start_offset, 0);
        assert_eq!(deser.end_offset, 6);
    }
}
