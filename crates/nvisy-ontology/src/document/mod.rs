//! [`Document`] — the unified addressable view of any processed
//! document, parameterised by modality.
//!
//! Native text (PDF text layers, DOCX runs, plain text), recognized
//! text (OCR'd images, transcribed audio), and tabular cells all
//! flow into the same shape: a [`Document<M>`] holding ordered
//! [`Block<M>`]s, user annotations, and an embedded [`Audit<M>`]
//! that accumulates the run's findings (detected entities) and
//! processing log (redaction entries).
//!
//! `Block<M>` is the universal wrapper carrying the common per-block
//! fields (spans, confidence). The modality-specific payload
//! (text+spans, region, time span, row coordinates) lives in
//! [`Modality::Block`] inside `block.kind`. Detected entities are
//! run-scoped and live on the document's [`Audit`], not on blocks.
//!
//! Rich sources (PDFs with both text and image layers) decompose
//! into multiple `Document<M>` values at the engine boundary, one
//! per modality. There is no cross-modality `Document`.
//!
//! `Document` is an in-memory pipeline carrier — it is intentionally
//! not `Serialize`/`Deserialize` and has no `Default`. The persisted
//! shape is the embedded [`Audit`], reached via [`AnyAudit`].
//!
//! [`AnyAudit`]: crate::provenance::AnyAudit
//!
//! [`Modality::Block`]: crate::modality::Modality::Block
//! [`Audit`]: crate::provenance::Audit

mod block;
mod span;

pub use self::block::Block;
pub use self::span::Span;
use crate::entity::{Annotation, ContentSource};
use crate::modality::Modality;
use crate::provenance::Audit;

/// Unified addressable view of a parsed document for modality `M`.
#[derive(Debug, Clone)]
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
    /// Provenance of processing for this document: detected
    /// entities and per-redaction audit entries. Travels with the
    /// document because every artifact a run produces about the
    /// document belongs *to* the document.
    pub audit: Audit<M>,
}

impl<M: Modality> Document<M> {
    /// Construct an empty [`Document`] for the given source with
    /// default metadata. Blocks and annotations start empty; the
    /// embedded [`Audit`] is initialised against the same source.
    /// Producers push blocks onto `self.blocks` directly.
    ///
    /// Use [`with_meta`] when the metadata isn't the per-modality
    /// default.
    ///
    /// [`with_meta`]: Self::with_meta
    pub fn new(source: ContentSource) -> Self {
        Self::with_meta(source, M::Metadata::default())
    }

    /// Construct an empty [`Document`] for the given source with
    /// explicit metadata. See [`new`] for the default-metadata
    /// shortcut.
    ///
    /// [`new`]: Self::new
    pub fn with_meta(source: ContentSource, meta: M::Metadata) -> Self {
        Self {
            meta,
            blocks: Vec::new(),
            annotations: Vec::new(),
            audit: Audit::new(source),
        }
    }
}
