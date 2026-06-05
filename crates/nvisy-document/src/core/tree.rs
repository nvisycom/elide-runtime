//! [`DocumentTree<M>`]: per-source, per-modality carrier the pipeline
//! operates on. Owns its codec handle directly — no lock, no
//! modality erasure.
//!
//! Modality erasure exists only at the import boundary in
//! [`AnyTree`]. The orchestrator matches `AnyTree` once and dispatches
//! into the typed [`DocumentTree<M>`]-shaped pipeline; from that
//! point on every phase body is `<M: DocumentModality>`-generic.
//!
//! Rich sources (PDF/DOCX) land as [`AnyTree::Text`] whose `embeds`
//! field carries the extracted image children as [`AnyTree::Image`]
//! entries. The orchestrator walks `embeds` recursively after the
//! root pipeline finishes.

use nvisy_codec::DocumentHandle;
use nvisy_core::content::ContentMetadata;
use nvisy_core::modality::{Audio, Image, ModalityKind, Tabular, Text};

use crate::document::Document;
use crate::modality::DocumentModality;
use crate::provenance::AnyAudit;

/// Typed per-modality tree. One per imported file (or per embedded
/// child of a rich import).
pub struct DocumentTree<M: DocumentModality> {
    /// The modality-typed document with its blocks + audit.
    pub root: Document<M>,
    /// Codec handle backing the underlying bytes. Phases borrow this
    /// directly via the typed [`DocumentHandle<M>`] for reads
    /// (via [`IndexedHandle::read`][r]) and redactions
    /// (via [`IndexedHandle::redact`][rd]).
    ///
    /// [r]: nvisy_codec::core::IndexedHandle::read
    /// [rd]: nvisy_codec::core::IndexedHandle::redact
    pub handle: DocumentHandle<M>,
    /// Content metadata (MIME type, filename, …) from the original
    /// upload. Policy evaluation matches rules against fields here.
    pub metadata: ContentMetadata,
    /// Nested embedded children. Populated for rich text sources
    /// (PDF, DOCX) by the importer, who pulls embedded image
    /// children out of the rich handler at import time. Empty for
    /// non-rich modalities.
    pub embeds: Vec<AnyTree>,
}

impl<M: DocumentModality> DocumentTree<M> {
    /// Construct a new typed tree from its parts. `embeds` starts empty;
    /// the importer is responsible for populating it for rich sources.
    pub fn new(root: Document<M>, handle: DocumentHandle<M>, metadata: ContentMetadata) -> Self {
        Self {
            root,
            handle,
            metadata,
            embeds: Vec::new(),
        }
    }
}

/// Modality-erased tree, used only at the import boundary and during
/// the orchestrator's modality dispatch. After dispatch, phases see
/// [`DocumentTree<M>`] directly.
#[non_exhaustive]
pub enum AnyTree {
    Text(DocumentTree<Text>),
    Tabular(DocumentTree<Tabular>),
    Image(DocumentTree<Image>),
    Audio(DocumentTree<Audio>),
}

impl AnyTree {
    /// Runtime modality tag.
    pub fn modality(&self) -> ModalityKind {
        match self {
            Self::Text(_) => ModalityKind::Text,
            Self::Tabular(_) => ModalityKind::Tabular,
            Self::Image(_) => ModalityKind::Image,
            Self::Audio(_) => ModalityKind::Audio,
        }
    }

    /// Stable lowercase name of the tree's modality, for telemetry.
    pub fn modality_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tabular(_) => "tabular",
            Self::Image(_) => "image",
            Self::Audio(_) => "audio",
        }
    }

    /// Clone the root document's audit as a modality-erased
    /// [`AnyAudit`] for the run summary collector.
    pub fn audit_cloned(&self) -> AnyAudit {
        match self {
            Self::Text(t) => t.root.audit.clone().into(),
            Self::Tabular(t) => t.root.audit.clone().into(),
            Self::Image(t) => t.root.audit.clone().into(),
            Self::Audio(t) => t.root.audit.clone().into(),
        }
    }

    /// Iterate the root tree's embedded children in document order.
    pub fn embeds(&self) -> &[AnyTree] {
        match self {
            Self::Text(t) => &t.embeds,
            Self::Tabular(t) => &t.embeds,
            Self::Image(t) => &t.embeds,
            Self::Audio(t) => &t.embeds,
        }
    }

    /// Mutable iterator over embedded children.
    pub fn embeds_mut(&mut self) -> &mut Vec<AnyTree> {
        match self {
            Self::Text(t) => &mut t.embeds,
            Self::Tabular(t) => &mut t.embeds,
            Self::Image(t) => &mut t.embeds,
            Self::Audio(t) => &mut t.embeds,
        }
    }
}
