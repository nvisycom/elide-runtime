//! [`Document`] — the unified addressable view of any processed
//! document, parameterised by modality.
//!
//! Native text (PDF text layers, DOCX runs, plain text), recognized
//! text (OCR'd images, transcribed audio), and tabular cells all
//! flow into the same shape: a [`Document<M>`] holding ordered
//! [`Block<M>`]s, each with a flat `text` string and a set of
//! [`Span<M>`]s mapping ranges of that text back to source coordinates
//! of modality `M`.
//!
//! Per-modality data (block classifications, non-textual artefacts,
//! document-level metadata) is carried through associated types on
//! [`Modality`] — see [`Modality::BlockKind`], [`Modality::Artefact`],
//! [`Modality::Metadata`].
//!
//! Heterogeneous sources (e.g. a PDF mixing image pages and a native
//! text layer) are represented as multiple per-modality `Document<M>`
//! values at the engine boundary, joined at the audit boundary via
//! [`AnyModality`].
//!
//! `Document` is an in-memory pipeline carrier — it is intentionally
//! not `Serialize`/`Deserialize` and has no `Default`. Persisted shapes
//! live elsewhere (audit, redaction map).
//!
//! [`Modality`]: crate::modality::Modality
//! [`Modality::BlockKind`]: crate::modality::Modality::BlockKind
//! [`Modality::Artefact`]: crate::modality::Modality::Artefact
//! [`Modality::Metadata`]: crate::modality::Modality::Metadata
//! [`AnyModality`]: crate::modality::AnyModality

mod block;
mod span;

pub use self::block::Block;
pub use self::span::Span;
use crate::modality::Modality;

/// Unified addressable view of a parsed document for modality `M`.
#[derive(Debug, Clone, PartialEq)]
pub struct Document<M: Modality> {
    /// Per-modality document-level metadata.
    pub meta: M::Metadata,
    /// Ordered blocks. One per page, paragraph, speaker turn, row,
    /// or just one for documents with no inherent block structure.
    pub blocks: Vec<Block<M>>,
}

impl<M: Modality> Document<M> {
    /// Construct a [`Document`] from its metadata and blocks.
    pub fn new(meta: M::Metadata, blocks: Vec<Block<M>>) -> Self {
        Self { meta, blocks }
    }

    /// Total character count across all blocks (sum of each block's
    /// `text.chars().count()`).
    pub fn char_count(&self) -> usize {
        self.blocks.iter().map(|b| b.text.chars().count()).sum()
    }

    /// Iterator over every span in every block, in block order.
    pub fn spans(&self) -> impl Iterator<Item = (&Block<M>, &Span<M>)> {
        self.blocks
            .iter()
            .flat_map(|b| b.spans.iter().map(move |s| (b, s)))
    }
}
