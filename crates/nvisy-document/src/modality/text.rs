//! Text-modality document shape: [`TextBlock`], [`TextContent`],
//! [`EmbeddedDocument`], [`TextMetadata`].

use nvisy_core::modality::{Image, Tabular, TextExtraction};
use nvisy_core::primitive::LanguageDetection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ModalityBlock;
use crate::document::Document;

/// Per-modality block payload for [`Text`].
///
/// Splits into two variants:
///
/// - [`Text`] wraps a structural text-shaped kind
///   (paragraph, heading, list item, code, quote) — see [`TextContent`].
/// - [`Embed`] hosts a nested [`Document`] of another
///   modality (image, tabular) for sources like PDF, DOCX, or HTML
///   that mix text with non-text content in one flow.
///
/// Per-word source spans live on the wrapping [`Block<Text>`].
///
/// [`Text`]: Self::Text
/// [`Embed`]: Self::Embed
/// [`Block<Text>`]: crate::document::Block
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TextBlock {
    /// Text-shaped content (paragraph, heading, list item, code, quote).
    Text(TextContent),
    /// A nested document of another modality (image, tabular) embedded
    /// in the text flow. Boxed because [`EmbeddedDocument`] wraps a
    /// full [`Document<M>`] (blocks + audit) which dwarfs the
    /// [`TextContent`] variant; without the box every `TextBlock`
    /// pays the embed footprint.
    Embed(Box<EmbeddedDocument>),
}

/// Text-shaped block content — paragraphs, headings, and other
/// structural variants that carry flat text.
///
/// Split out from [`TextBlock`] so the embed variants stay distinct
/// and text-only recognizers can match on `TextBlock::Text(_)` in one
/// arm without forgetting embed variants.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TextContent {
    /// A regular paragraph or text run.
    Paragraph { text: String },
    /// A heading.
    Heading {
        text: String,
        /// Heading depth (1 = h1, 2 = h2, …). `None` when the source
        /// doesn't expose a level.
        level: Option<u8>,
    },
    /// A list item.
    ListItem { text: String },
    /// A code block or pre-formatted text.
    Code { text: String },
    /// A blockquote.
    Quote { text: String },
}

impl TextContent {
    /// The content's text.
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

/// A nested document of another modality embedded inside a
/// [`TextBlock::Embed`] variant.
///
/// PDFs and other rich containers can host images and tables inline
/// with their text flow. The engine processes each nested document
/// through its own per-modality phase chain; the recursion is
/// orchestrator-driven so individual phases stay single-doc.
///
/// Recursion is one-directional: only text can host other modalities.
/// `ImageBlock`, `AudioBlock`, and `TabularBlock` have no embed
/// variants by construction, so the maximum nesting depth is 2 and
/// no termination check is needed at runtime.
#[derive(Debug, Clone)]
pub enum EmbeddedDocument {
    /// A nested image document (e.g. a PDF figure or DOCX picture).
    Image(Document<Image>),
    /// A nested tabular document (e.g. a PDF table or DOCX table).
    Tabular(Document<Tabular>),
}

impl ModalityBlock for TextBlock {
    fn scan_text(&self) -> Option<&str> {
        match self {
            Self::Text(content) => Some(content.text()),
            Self::Embed(_) => None,
        }
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
