//! [`Document`] — the unified addressable view of any processed
//! document, parameterised by modality.
//!
//! Native text (PDF text layers, DOCX runs, plain text), recognized
//! text (OCR'd images, transcribed audio), and tabular cells all
//! flow into the same shape: a [`Document<M>`] holding ordered
//! [`Block<M>`]s plus user annotations. The document type is
//! intentionally structural-only — it describes what the source
//! material contains. Run-scoped provenance (detected entities,
//! redaction audit entries) lives on the engine's `DocumentEnvelope`
//! and never on the document itself.
//!
//! `Block<M>` is the universal wrapper carrying the common per-block
//! fields (spans, confidence). The modality-specific payload
//! (text+spans, region, time span, row coordinates) lives in
//! [`Modality::Block`] inside `block.kind`.
//!
//! Rich sources (PDFs with both text and image layers) decompose
//! into multiple `Document<M>` values at the engine boundary, one
//! per modality. There is no cross-modality `Document`.
//!
//! `Document` is an in-memory pipeline carrier — it is intentionally
//! not `Serialize`/`Deserialize` and has no `Default`. The persisted
//! shape is the run-scoped [`Audit`], owned by the engine's
//! `DocumentEnvelope` and erased on the wire via [`AnyAudit`].
//!
//! [`Audit`]: crate::provenance::Audit
//! [`AnyAudit`]: crate::provenance::AnyAudit
//!
//! [`Modality::Block`]: crate::modality::Modality::Block

mod block;
mod span;

pub use self::block::Block;
pub use self::span::Span;
use crate::entity::{Annotation, LabelAnnotation};
use crate::modality::Modality;

/// Unified addressable view of a parsed document for modality `M`.
///
/// Purely structural: what the source material contains. Run-scoped
/// provenance (detected entities, redaction audit entries) lives on
/// the engine's `DocumentEnvelope<M>` instead, so the same parsed
/// document can in principle participate in multiple runs without
/// the document type carrying per-run state.
#[derive(Debug, Clone)]
pub struct Document<M: Modality> {
    /// Per-modality document-level metadata.
    pub meta: M::Metadata,
    /// Ordered blocks. One per page, paragraph, speaker turn, row,
    /// or just one for documents with no inherent block structure.
    pub blocks: Vec<Block<M>>,

    /// User-supplied region annotations (inclusions and exclusions)
    /// attached at upload time. Annotation locations target source
    /// coordinates within the document.
    pub annotations: Vec<Annotation<M>>,
    /// Document-level classification labels. Modality-agnostic and
    /// propagated to every envelope spawned from the same source so
    /// policy rules that condition on labels can fire uniformly.
    pub labels: Vec<LabelAnnotation>,
}

impl<M: Modality> Document<M> {
    /// Construct an empty [`Document`] with explicit metadata.
    /// Blocks, annotations, and labels start empty; producers push
    /// blocks onto `self.blocks` directly.
    ///
    /// The importer always knows which extraction path produced the
    /// document, so the metadata is required at construction time
    /// rather than defaulted.
    pub fn new(meta: M::Metadata) -> Self {
        Self {
            meta,
            blocks: Vec::new(),
            annotations: Vec::new(),
            labels: Vec::new(),
        }
    }
}
