//! Text modality.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Modality, ModalityBlock, Overlap, TextExtraction};
use crate::policy::TextStrategy;
use crate::primitive::LanguageDetection;

/// Half-open `[start, end)` byte range around a [`Text`] location,
/// used for the optional surrounding context window. The newtype
/// makes the "both endpoints or none" invariant unrepresentable —
/// the previous twin-`Option` fields allowed a `(Some, None)`
/// half-state with no meaningful semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextWindow {
    /// Byte offset where the context window starts.
    pub start: usize,
    /// Byte offset where the context window ends (exclusive).
    pub end: usize,
}

impl ContextWindow {
    /// Construct a window covering `start..end`.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A range within text content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    /// Byte or character offset where the range starts.
    pub start: usize,
    /// Byte or character offset where the range ends.
    pub end: usize,
    /// Surrounding context window for redaction, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextWindow>,
    /// 1-based page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
}

impl Text {
    /// Create a [`Text`] covering `start..end` with all optional
    /// fields unset.
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            end,
            context: None,
            page_number: None,
        }
    }

    /// Byte length of the range (`end - start`).
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the range is empty (zero length).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Modality for Text {
    type Block = TextBlock;
    type Metadata = TextMetadata;
    type Replacement = crate::provenance::TextReplacement;
    type Strategy = TextStrategy;
}

/// Per-modality block payload for [`Text`]. Each variant is a
/// structural kind (paragraph, heading, list item, code, quote);
/// every variant carries flat text. Per-word source spans live on
/// the wrapping [`Block<Text>`].
///
/// [`Block<Text>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TextBlock {
    /// A regular paragraph or text run.
    Paragraph { text: String },
    /// A heading.
    Heading {
        text: String,
        /// Heading depth (1 = h1, 2 = h2, …). `None` when the source
        /// doesn't expose a level.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<u8>,
    },
    /// A list item.
    ListItem { text: String },
    /// A code block or pre-formatted text.
    Code { text: String },
    /// A blockquote.
    Quote { text: String },
}

impl TextBlock {
    /// The block's text.
    pub fn text(&self) -> &str {
        match self {
            Self::Paragraph { text }
            | Self::Heading { text, .. }
            | Self::ListItem { text }
            | Self::Code { text }
            | Self::Quote { text } => text,
        }
    }
}

impl ModalityBlock for TextBlock {
    fn scan_text(&self) -> Option<&str> {
        Some(self.text())
    }
}

/// Document-level metadata for [`Document<Text>`].
///
/// [`Document<Text>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextMetadata {
    /// How this document's text was produced (native text-layer parse
    /// vs OCR'd image-backed page).
    pub extraction: TextExtraction,
    /// Languages detected (or asserted) for the document content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
}

impl From<TextExtraction> for TextMetadata {
    /// Build [`TextMetadata`] carrying only the importer-known
    /// extraction tag. All other fields start empty; downstream
    /// stages fill them in as they discover the document's content.
    fn from(extraction: TextExtraction) -> Self {
        Self {
            extraction,
            languages: Vec::new(),
        }
    }
}

impl Overlap for Text {
    /// Two text ranges overlap only when they share a page (or both
    /// have `page_number: None`) and their byte ranges intersect.
    /// Without the page gate, two ranges on different pages of the
    /// same document that happen to share byte offsets would
    /// false-positive as overlapping.
    fn overlaps(&self, other: &Self) -> bool {
        self.page_number == other.page_number && self.start < other.end && other.start < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn len_and_is_empty() {
        assert_eq!(Text::new(0, 10).len(), 10);
        assert!(!Text::new(0, 10).is_empty());
        assert!(Text::new(5, 5).is_empty());
    }

    #[test]
    fn overlap_intersecting() {
        assert!(Text::new(0, 10).overlaps(&Text::new(5, 15)));
    }

    #[test]
    fn overlap_contained() {
        assert!(Text::new(0, 10).overlaps(&Text::new(2, 5)));
    }

    #[test]
    fn no_overlap_adjacent() {
        assert!(!Text::new(0, 5).overlaps(&Text::new(5, 10)));
    }

    #[test]
    fn no_overlap_disjoint() {
        assert!(!Text::new(0, 5).overlaps(&Text::new(10, 15)));
    }
}
