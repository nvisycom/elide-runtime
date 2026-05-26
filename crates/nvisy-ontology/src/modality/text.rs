//! Text modality.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Mergeable, Modality, Overlap};
use crate::document::Span;
use crate::primitive::LanguageDetection;

/// A range within text content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "TextBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    /// Byte or character offset where the range starts.
    pub start_offset: usize,
    /// Byte or character offset where the range ends.
    pub end_offset: usize,
    /// Start offset of the surrounding context window for redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_start_offset: Option<usize>,
    /// End offset of the surrounding context window for redaction.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_end_offset: Option<usize>,
    /// 1-based page number.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number: Option<u32>,
    /// 1-based line number.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

impl Text {
    /// Create a [`Text`] covering `start_offset..end_offset` with all
    /// optional fields unset.
    pub fn new(start_offset: usize, end_offset: usize) -> Self {
        Self {
            start_offset,
            end_offset,
            context_start_offset: None,
            context_end_offset: None,
            page_number: None,
            line_number: None,
        }
    }

    /// Create a new [`TextBuilder`].
    pub fn builder() -> TextBuilder {
        TextBuilder::default()
    }

    /// Byte length of the range (`end_offset - start_offset`).
    pub fn len(&self) -> usize {
        self.end_offset.saturating_sub(self.start_offset)
    }

    /// Whether the range is empty (zero length).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Modality for Text {
    type Block = TextBlock;
    type Metadata = TextMetadata;
}

/// Per-modality block payload for [`Text`]. Each variant is a
/// structural kind (paragraph, heading, list item, code, quote);
/// every variant carries flat text plus per-word [`Span<Text>`]s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TextBlock {
    /// A regular paragraph or text run.
    Paragraph {
        text: String,
        spans: Vec<Span<Text>>,
    },
    /// A heading.
    Heading {
        text: String,
        spans: Vec<Span<Text>>,
        /// Heading depth (1 = h1, 2 = h2, …). `None` when the source
        /// doesn't expose a level.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<u8>,
    },
    /// A list item.
    ListItem {
        text: String,
        spans: Vec<Span<Text>>,
    },
    /// A code block or pre-formatted text.
    Code {
        text: String,
        spans: Vec<Span<Text>>,
    },
    /// A blockquote.
    Quote {
        text: String,
        spans: Vec<Span<Text>>,
    },
}

impl TextBlock {
    /// The block's text.
    pub fn text(&self) -> &str {
        match self {
            Self::Paragraph { text, .. }
            | Self::Heading { text, .. }
            | Self::ListItem { text, .. }
            | Self::Code { text, .. }
            | Self::Quote { text, .. } => text,
        }
    }

    /// The block's spans (per-word source mapping).
    pub fn spans(&self) -> &[Span<Text>] {
        match self {
            Self::Paragraph { spans, .. }
            | Self::Heading { spans, .. }
            | Self::ListItem { spans, .. }
            | Self::Code { spans, .. }
            | Self::Quote { spans, .. } => spans,
        }
    }
}

/// Document-level metadata for [`Document<Text>`].
///
/// [`Document<Text>`]: crate::document::Document
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextMetadata {
    /// Languages detected (or asserted) for the document content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
}

impl Overlap for Text {
    fn overlaps(&self, other: &Self) -> bool {
        self.start_offset < other.end_offset && other.start_offset < self.end_offset
    }
}

impl Mergeable for Text {
    /// Merge two text ranges by unioning byte offsets when their
    /// non-range identity (page/line) matches. Context offsets union
    /// when present on both sides; otherwise the result has no context
    /// window.
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.page_number != other.page_number || self.line_number != other.line_number {
            return None;
        }
        Some(Self {
            start_offset: self.start_offset.min(other.start_offset),
            end_offset: self.end_offset.max(other.end_offset),
            context_start_offset: option_min(self.context_start_offset, other.context_start_offset),
            context_end_offset: option_max(self.context_end_offset, other.context_end_offset),
            page_number: self.page_number,
            line_number: self.line_number,
        })
    }
}

fn option_min(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        _ => None,
    }
}

fn option_max(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        _ => None,
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
