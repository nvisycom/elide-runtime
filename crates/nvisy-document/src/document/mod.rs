//! [`Document`] — the unified addressable view of any processed
//! document, parameterised by modality.
//!
//! Native text (PDF text layers, DOCX runs, plain text), recognized
//! text (OCR'd images, transcribed audio), and tabular cells all
//! flow into the same shape: a [`Document<M>`] holding ordered
//! [`Block<M>`]s plus user annotations and the run-scoped
//! [`Audit<M>`] that accrues detected entities and redaction entries
//! as the pipeline progresses.
//!
//! `Block<M>` is the universal wrapper carrying the common per-block
//! fields (spans, confidence). The modality-specific payload
//! (text+spans, region, time span, row coordinates) lives in
//! `M::Block` inside `block.kind`.
//!
//! Rich sources (PDFs with both text and image layers) decompose
//! into a single recursive `Document<Text>` whose blocks can host
//! nested `Document<Image>` / `Document<Tabular>` children via
//! [`TextBlock::Embed`]. Each nested document carries its own typed
//! audit; the per-source audit is the tree of nested audits, with
//! the wire shape ([`AnyAudit`]) being one projection of it.
//!
//! `Document` is an in-memory pipeline carrier — it is intentionally
//! not `Serialize`/`Deserialize` and has no `Default`.
//!
//! [`Audit<M>`]: crate::provenance::Audit
//! [`AnyAudit`]: crate::provenance::AnyAudit
//! [`TextBlock::Embed`]: crate::modality::TextBlock::Embed

mod block;
mod span;

use nvisy_core::entity::{Annotation, ContentSource, Entity, LabelAnnotation};

pub use self::block::Block;
pub use self::span::Span;
use crate::provenance::Audit;

/// Unified addressable view of a parsed document for modality `M`.
///
/// Combines structural content (blocks, annotations, labels) with
/// the run-scoped [`Audit<M>`] that accumulates detected entities and
/// redaction records as the pipeline progresses. The audit lives
/// here so the tree-shaped recursive document model (a
/// `Document<Text>` whose blocks embed `Document<Image>` /
/// `Document<Tabular>`) keeps every nested doc's audit attached to
/// the doc it describes — the per-source audit is the tree of these.
///
/// [`Audit<M>`]: crate::provenance::Audit
#[derive(Debug, Clone)]
pub struct Document<M: crate::modality::DocumentModality + nvisy_toolkit::redaction::Redactable> {
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

    /// Run-scoped provenance for this document: detected entities
    /// and per-redaction audit entries. Opens empty against the
    /// document's source at construction; each pipeline phase
    /// (detection, redaction, validation) reads from and writes to
    /// it as it executes.
    pub audit: Audit<M>,
}

impl<M: crate::modality::DocumentModality + nvisy_toolkit::redaction::Redactable> Document<M> {
    /// Construct an empty [`Document`] with explicit metadata and a
    /// fresh audit opened against `source`. Blocks, annotations, and
    /// labels start empty; producers push onto them directly.
    ///
    /// The importer always knows which extraction path produced the
    /// document, so the metadata is required at construction time
    /// rather than defaulted.
    pub fn new(meta: M::Metadata, source: ContentSource) -> Self {
        Self {
            meta,
            blocks: Vec::new(),
            annotations: Vec::new(),
            labels: Vec::new(),
            audit: Audit::new(source),
        }
    }

    /// Append detected entities to the document's audit, wrapping
    /// each into a fresh [`EntityRecord`]. Used by detection phases
    /// at both the root and nested-document level.
    ///
    /// [`EntityRecord`]: crate::provenance::EntityRecord
    pub fn add_entities(&mut self, entities: impl IntoIterator<Item = Entity<M>>) {
        for entity in entities {
            self.audit.push_entity(entity);
        }
    }
}
