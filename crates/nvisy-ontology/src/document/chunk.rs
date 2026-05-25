//! [`Chunk`] — one logical piece of a [`Document`] with its own
//! flat text and source-mapped spans.
//!
//! [`Document`]: super::Document

use std::ops::Range;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Span;
use crate::primitive::TimeSpan;

/// One chunk of a [`Document`]: a page, row, or the whole
/// document if it has no inherent chunking.
///
/// `text` is the flat string detection runs on. `spans` carries
/// the per-recognized-unit ranges back to their source locations.
/// Spans are ordered by `text_start` and cover the regions of
/// `text` that came from somewhere addressable (whitespace and
/// joiners between spans may not be covered).
///
/// [`Document`]: super::Document
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    /// Flat text of this chunk. Detection runs on this string.
    pub text: String,
    /// Spans within `text`, in `text_start` order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<Span>,
    /// Chunk-level metadata describing what kind of piece this is.
    pub meta: ChunkMeta,
}

impl Chunk {
    /// Returns the first span whose range contains `offset`, if any.
    ///
    /// Half-open semantics: a span covers `text_start..text_end`,
    /// so an offset equal to `text_end` is *not* contained.
    pub fn span_at(&self, offset: usize) -> Option<&Span> {
        self.spans.iter().find(|s| s.contains(offset))
    }

    /// Iterator over every span overlapping `range`.
    pub fn spans_in(&self, range: Range<usize>) -> impl Iterator<Item = &Span> {
        self.spans
            .iter()
            .filter(move |s| s.overlaps(range.start, range.end))
    }
}

/// Metadata describing what kind of chunk this is and the
/// modality-specific coordinates that locate the chunk in its
/// source.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChunkMeta {
    /// A page from a paged format (image, PDF). `number` is
    /// 1-based; `width` and `height` are pixel dimensions when
    /// known.
    Page {
        number: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<f64>,
    },
    /// A row from a tabular document. `index` is 0-based.
    Row { index: u32 },
    /// A segment from an audio transcription. Carries the time
    /// span and optional speaker identifier from diarization.
    AudioSegment {
        time_span: TimeSpan,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speaker_id: Option<String>,
    },
    /// A document with no inherent chunking (plain text, JSON,
    /// Markdown).
    Document,
}

/// A column header in a tabular document.
///
/// Lives on [`super::DocumentMeta`] because headers apply across
/// all rows rather than to a single chunk.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ColumnHeader {
    /// 0-based column index.
    pub column_index: u32,
    /// Header text.
    pub text: String,
}
