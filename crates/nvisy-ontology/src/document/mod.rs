//! [`Document`] — the unified addressable view of any processed
//! document, regardless of source modality.
//!
//! Both natively-extracted text (PDF text layers, DOCX runs, plain
//! text) and recognized text (OCR'd images, transcribed audio,
//! tabular cells) flow into the same shape: a list of [`Chunk`]s,
//! each carrying a flat text string and a set of [`Span`]s that
//! map ranges of that text back to their source [`Location`]s.
//!
//! Detection runs on the flat text. Redaction reads the source
//! [`Location`] of each detected span and dispatches the right
//! per-modality edit. The modality lives in the location, not in
//! the document shape — a single document can contain pages
//! recognized by OCR and pages extracted from a native text layer,
//! mixed freely.
//!
//! This module replaces the per-modality artifacts under
//! [`crate::artifacts`]. New code should populate and consume
//! [`Document`]; the artifacts module will be removed in a follow-up
//! once the migration is complete.
//!
//! [`Location`]: crate::entity::Location

mod chunk;
mod span;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::chunk::{Chunk, ChunkMeta, ColumnHeader};
pub use self::span::Span;
use crate::primitive::LanguageDetection;

/// Unified addressable view of a parsed document.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// Document-level metadata.
    #[serde(default)]
    pub meta: DocumentMeta,
    /// Ordered chunks. One per page (paged formats), row (tabular),
    /// or just one for documents with no inherent chunking (plain
    /// text, JSON, Markdown).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<Chunk>,
}

impl Document {
    /// Create an empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total character count across all chunks (sum of each chunk's
    /// `text.chars().count()`).
    pub fn char_count(&self) -> usize {
        self.chunks.iter().map(|c| c.text.chars().count()).sum()
    }

    /// Iterator over every span in every chunk, in chunk order.
    pub fn spans(&self) -> impl Iterator<Item = (&Chunk, &Span)> {
        self.chunks
            .iter()
            .flat_map(|c| c.spans.iter().map(move |s| (c, s)))
    }
}

/// Document-level metadata: things that apply to the whole
/// document rather than a single chunk.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMeta {
    /// Languages detected (or asserted) for the document content.
    ///
    /// Multiple entries when the document is mixed-language: each
    /// [`LanguageDetection`] carries its own provenance, optional
    /// confidence, and optional byte-offset [`LanguageSpan`] within
    /// the chunk text. Empty when language is unknown or wasn't
    /// resolved at parse time.
    ///
    /// [`LanguageSpan`]: crate::primitive::LanguageSpan
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
    /// Sparse list of column headers, present only for tabular
    /// documents. Empty otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<ColumnHeader>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Location, TextLocation};

    #[test]
    fn empty_document_has_zero_char_count() {
        let doc = Document::new();
        assert_eq!(doc.char_count(), 0);
        assert!(doc.chunks.is_empty());
    }

    #[test]
    fn char_count_sums_across_chunks() {
        let chunk = |s: &str| Chunk {
            text: s.to_owned(),
            spans: vec![Span {
                text_start: 0,
                text_end: s.len(),
                confidence: None,
                source: Location::Text(TextLocation::new(0, s.len())),
            }],
            meta: ChunkMeta::Document,
        };
        let doc = Document {
            meta: DocumentMeta::default(),
            chunks: vec![chunk("hello"), chunk("world!")],
        };
        assert_eq!(doc.char_count(), "hello".chars().count() + "world!".chars().count());
    }

    #[test]
    fn spans_iterator_visits_every_span() {
        let mk_chunk = |spans: Vec<Span>| Chunk {
            text: String::new(),
            spans,
            meta: ChunkMeta::Document,
        };
        let mk_span = |start, end| Span {
            text_start: start,
            text_end: end,
            confidence: None,
            source: Location::Text(TextLocation::new(start, end)),
        };
        let doc = Document {
            meta: DocumentMeta::default(),
            chunks: vec![
                mk_chunk(vec![mk_span(0, 5), mk_span(6, 11)]),
                mk_chunk(vec![mk_span(0, 4)]),
            ],
        };
        assert_eq!(doc.spans().count(), 3);
    }
}
