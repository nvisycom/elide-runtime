//! [`Document`] — the unified addressable view of any processed
//! document, parameterised by modality.
//!
//! Native text (PDF text layers, DOCX runs, plain text), recognized
//! text (OCR'd images, transcribed audio), and tabular cells all
//! flow into the same shape: a [`Document<M>`] holding ordered
//! blocks (per-modality via [`Modality::Block`]), detected entities,
//! user annotations, and document-level metadata.
//!
//! Each modality defines its own block shape — see [`TextBlock`],
//! [`ImageBlock`], [`AudioBlock`], [`TabularBlock`] — so per-modality
//! payloads diverge cleanly (an audio block carries time spans and
//! speaker, an image block carries a region per variant, etc.).
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
//! [`TextBlock`]: crate::modality::TextBlock
//! [`ImageBlock`]: crate::modality::ImageBlock
//! [`AudioBlock`]: crate::modality::AudioBlock
//! [`TabularBlock`]: crate::modality::TabularBlock

mod span;

pub use self::span::Span;
use crate::entity::{Annotation, Entity};
use crate::modality::Modality;

/// Unified addressable view of a parsed document for modality `M`.
#[derive(Debug, Clone, PartialEq)]
pub struct Document<M: Modality> {
    /// Per-modality document-level metadata.
    pub meta: M::Metadata,
    /// Ordered blocks. One per page, paragraph, speaker turn, row,
    /// or just one for documents with no inherent block structure.
    pub blocks: Vec<M::Block>,
    /// Entities detected within this document by recognizers.
    pub entities: Vec<Entity<M>>,
    /// User-supplied annotations (inclusions, exclusions, labels)
    /// attached at upload time.
    pub annotations: Vec<Annotation<M>>,
}

impl<M: Modality> Document<M> {
    /// Construct a [`Document`] with the given metadata and blocks.
    /// Entities and annotations start empty.
    pub fn new(meta: M::Metadata, blocks: Vec<M::Block>) -> Self {
        Self {
            meta,
            blocks,
            entities: Vec::new(),
            annotations: Vec::new(),
        }
    }
}
