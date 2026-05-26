//! [`Block`] — one logical region of a [`Document`].

use std::ops::Range;

use super::span::Span;
use crate::modality::Modality;
use crate::primitive::Confidence;

/// A logical region within a [`Document`]: a paragraph of native text,
/// an OCR'd page or text block, a speaker turn, or a tabular row.
///
/// `text` is the flat string detection runs on. `spans` carries the
/// per-recognized-unit ranges back to their source coordinates.
/// `artefacts` records non-textual elements detected in the same
/// region; the shape is per-modality via [`Modality::Artefact`].
///
/// [`Document`]: super::Document
#[derive(Debug, Clone, PartialEq)]
pub struct Block<M: Modality> {
    /// Flat text of this block. Detection runs on this string.
    pub text: String,
    /// Spans within `text`, in `text_start` order.
    pub spans: Vec<Span<M>>,
    /// Per-modality classification of this block.
    pub kind: M::BlockKind,
    /// Recognition confidence for the block as a whole.
    pub confidence: Option<Confidence>,
    /// Source coordinates of this block in the original document.
    pub source: M,
    /// Non-textual elements detected in this block; per-modality shape
    /// via [`Modality::Artefact`].
    pub artefacts: Vec<M::Artefact>,
}

impl<M: Modality> Block<M> {
    /// Returns the first span whose range contains `offset`, if any.
    pub fn span_at(&self, offset: usize) -> Option<&Span<M>> {
        self.spans.iter().find(|s| s.contains(offset))
    }

    /// Iterator over every span overlapping `range`.
    pub fn spans_in(&self, range: Range<usize>) -> impl Iterator<Item = &Span<M>> {
        self.spans
            .iter()
            .filter(move |s| s.overlaps(range.start, range.end))
    }
}
