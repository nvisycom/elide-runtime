//! [`Document`] — the unified addressable view of any processed
//! document, parameterised by modality.
//!
//! Native text (PDF text layers, DOCX runs, plain text), recognized
//! text (OCR'd images, transcribed audio), and tabular cells all
//! flow into the same shape: a [`Document<M>`] holding ordered
//! [`Block<M>`]s and user annotations.
//!
//! `Block<M>` is the universal wrapper carrying the common per-block
//! fields (confidence, entities). The modality-specific payload
//! (text+spans, region, time span, row coordinates) lives in
//! [`Modality::Block`] inside `block.kind`.
//!
//! Rich sources (PDFs with both text and image layers) decompose
//! into multiple `Document<M>` values at the engine boundary, one
//! per modality. There is no cross-modality `Document`.
//!
//! `Document` is an in-memory pipeline carrier — it is intentionally
//! not `Serialize`/`Deserialize` and has no `Default`. Persisted
//! shapes live elsewhere (audit, redaction map).
//!
//! [`Modality::Block`]: crate::modality::Modality::Block

mod block;
mod span;

pub use self::block::Block;
pub use self::span::Span;
use crate::entity::Annotation;
use crate::modality::Modality;

/// Unified addressable view of a parsed document for modality `M`.
#[derive(Debug, Clone, PartialEq)]
pub struct Document<M: Modality> {
    /// Per-modality document-level metadata.
    pub meta: M::Metadata,
    /// Ordered blocks. One per page, paragraph, speaker turn, row,
    /// or just one for documents with no inherent block structure.
    pub blocks: Vec<Block<M>>,
    /// User-supplied annotations (inclusions, exclusions, labels)
    /// attached at upload time. Annotation locations target source
    /// coordinates within the document.
    pub annotations: Vec<Annotation<M>>,
}

impl<M: Modality> Document<M> {
    /// Construct a [`Document`] with the given metadata and blocks.
    /// Annotations start empty.
    pub fn new(meta: M::Metadata, blocks: Vec<Block<M>>) -> Self {
        Self {
            meta,
            blocks,
            annotations: Vec::new(),
        }
    }
}
