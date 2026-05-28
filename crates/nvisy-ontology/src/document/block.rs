//! [`Block`] — universal wrapper around a per-modality block payload.

use super::Span;
use crate::modality::Modality;
use crate::primitive::Confidence;

/// One block of a [`Document<M>`].
///
/// Universal across modalities: `kind` carries the modality-specific
/// payload (variant + its location-specific data) via [`M::Block`],
/// while `spans` and `confidence` are the common per-block
/// bookkeeping.
///
/// `spans` is empty for blocks that don't carry text (e.g. an image
/// `Figure` or `Logo`).
///
/// Detected entities don't live on the block — they're a run-scoped
/// finding and live on the document's [`Audit`].
///
/// [`Document<M>`]: super::Document
/// [`M::Block`]: crate::modality::Modality::Block
/// [`Audit`]: crate::provenance::Audit
#[derive(Debug, Clone, PartialEq)]
pub struct Block<M: Modality> {
    /// Modality-specific payload (variant + its data).
    pub kind: M::Block,
    /// Source-mapped spans into the block's text. Empty for non-
    /// textual blocks.
    pub spans: Vec<Span<M>>,
    /// Recognition confidence for the block as a whole. Absent for
    /// native text-layer extraction where the source already provides
    /// the text directly.
    pub confidence: Option<Confidence>,
}

impl<M: Modality> Block<M> {
    /// Construct a new block with empty spans and no confidence.
    pub fn new(kind: M::Block) -> Self {
        Self {
            kind,
            spans: Vec::new(),
            confidence: None,
        }
    }

    /// Set the source-mapped spans (builder-style).
    pub fn with_spans(mut self, spans: Vec<Span<M>>) -> Self {
        self.spans = spans;
        self
    }
}
