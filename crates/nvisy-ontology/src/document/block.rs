//! [`Block`] — universal wrapper around a per-modality block payload.

use crate::entity::Entity;
use crate::modality::Modality;
use crate::primitive::Confidence;

/// One block of a [`Document<M>`].
///
/// Universal across modalities: `kind` carries the modality-specific
/// payload (text+spans, region, time span, row coordinates) via
/// [`M::Block`], while `confidence` and `entities` are the common
/// per-block bookkeeping.
///
/// [`Document<M>`]: super::Document
/// [`M::Block`]: crate::modality::Modality::Block
#[derive(Debug, Clone, PartialEq)]
pub struct Block<M: Modality> {
    /// Modality-specific payload (variant + its data).
    pub kind: M::Block,
    /// Recognition confidence for the block as a whole. Absent for
    /// native text-layer extraction where the source already provides
    /// the text directly.
    pub confidence: Option<Confidence>,
    /// Entities detected within this block by recognizer passes.
    pub entities: Vec<Entity<M>>,
}

impl<M: Modality> Block<M> {
    /// Construct a new block with empty entities and no confidence.
    pub fn new(kind: M::Block) -> Self {
        Self {
            kind,
            confidence: None,
            entities: Vec::new(),
        }
    }

    /// Set the recognition confidence (builder-style).
    pub fn with_confidence(mut self, confidence: Confidence) -> Self {
        self.confidence = Some(confidence);
        self
    }
}
