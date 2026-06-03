//! [`Span`] — a range within a block's flat text, tagged with its
//! source coordinates.

use nvisy_core::modality::Modality;
use nvisy_core::primitive::Confidence;
use nvisy_toolkit::redaction::Redactable;

use crate::modality::DocumentModality;

/// A range of text within a block's flat `text`, paired with the
/// source coordinates of where that text came from.
///
/// Source-mapped granularity is typically one span per recognized
/// unit:
/// - one span per OCR word for image blocks,
/// - one span per cell for tabular rows,
/// - one span per transcribed word for audio,
/// - one span per text run for natively-extracted text.
#[derive(Debug, Clone, PartialEq)]
pub struct Span<M: DocumentModality + Redactable> {
    /// Byte offset into the block's `text` where this span starts.
    pub text_start: usize,
    /// Byte offset into the block's `text` where this span ends
    /// (exclusive).
    pub text_end: usize,
    /// Recognition confidence in this span. Populated for OCR and STT
    /// spans; absent for native text-layer extractions where the
    /// source already provides the text directly.
    pub confidence: Option<Confidence>,
    /// Source coordinates of where this text came from.
    pub source: <M as Modality>::Location,
}

impl<M: DocumentModality + Redactable> Span<M> {
    /// Byte length of the span in the block's `text`.
    pub fn len(&self) -> usize {
        self.text_end.saturating_sub(self.text_start)
    }

    /// Returns `true` when the span covers zero bytes.
    pub fn is_empty(&self) -> bool {
        self.text_end <= self.text_start
    }

    /// Returns `true` when `offset` falls within
    /// `text_start..text_end`.
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.text_start && offset < self.text_end
    }

    /// Returns `true` when this span's text range overlaps
    /// `[start, end)`.
    pub fn overlaps(&self, start: usize, end: usize) -> bool {
        self.text_start < end && self.text_end > start
    }
}
