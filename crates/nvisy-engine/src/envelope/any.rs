//! [`AnyEnvelope`]: modality-erased enum over [`DocumentEnvelope<M>`].
//!
//! The importer produces one [`AnyEnvelope`] per uploaded file.
//! Single-modality content (txt, csv, png, wav, …) yields the
//! matching variant. Rich documents (PDF, DOCX) yield the `Text`
//! variant; their image and tabular content lives inside the root
//! [`Document<Text>`] as nested documents under [`TextBlock::Embed`]
//! children, populated by the image/tabular extraction steps from
//! the same shared codec handle.
//!
//! [`Document<Text>`]: nvisy_ontology::document::Document
//! [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed

use derive_more::{From, IsVariant};
use nvisy_ontology::modality::{Audio, Image, Tabular, Text};
use nvisy_ontology::provenance::AnyAudit;

use super::{DocumentEnvelope, SharedHandle};

/// A modality-erased envelope returned by the importer.
#[derive(Debug, From, IsVariant)]
pub enum AnyEnvelope {
    Text(DocumentEnvelope<Text>),
    Tabular(DocumentEnvelope<Tabular>),
    Image(DocumentEnvelope<Image>),
    Audio(DocumentEnvelope<Audio>),
}

impl AnyEnvelope {
    /// Human-readable name of the contained modality. Used for
    /// telemetry and error messages.
    pub fn modality_name(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Tabular(_) => "tabular",
            Self::Image(_) => "image",
            Self::Audio(_) => "audio",
        }
    }

    /// Borrow the underlying shared codec handle without committing
    /// to a modality.
    pub fn handle(&self) -> &SharedHandle {
        match self {
            Self::Text(e) => &e.handle,
            Self::Tabular(e) => &e.handle,
            Self::Image(e) => &e.handle,
            Self::Audio(e) => &e.handle,
        }
    }

    /// Clone the envelope's audit as a modality-erased [`AnyAudit`].
    /// Used by the engine's run-summary collector so it doesn't have
    /// to match on every variant.
    pub fn audit_cloned(&self) -> AnyAudit {
        match self {
            Self::Text(e) => e.document.audit.clone().into(),
            Self::Tabular(e) => e.document.audit.clone().into(),
            Self::Image(e) => e.document.audit.clone().into(),
            Self::Audio(e) => e.document.audit.clone().into(),
        }
    }
}
