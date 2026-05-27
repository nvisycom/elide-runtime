//! [`AnyEnvelope`]: modality-erased enum over [`DocumentEnvelope<M>`].
//!
//! The importer produces one or more [`AnyEnvelope`]s per uploaded
//! file. Single-modality content (txt, csv, png, wav, …) yields one
//! entry; rich documents (PDF, DOCX) fan out into a `Text` and an
//! `Image` entry that share one `Arc<Mutex<DocumentHandle>>` so
//! reads and mutations stay coordinated.

use derive_more::IsVariant;
use nvisy_ontology::modality::{Audio, Image, Tabular, Text};

use super::DocumentEnvelope;

/// A modality-erased envelope returned by the importer.
#[derive(Debug, IsVariant)]
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
    pub fn handle(&self) -> &super::SharedHandle {
        match self {
            Self::Text(e) => &e.handle,
            Self::Tabular(e) => &e.handle,
            Self::Image(e) => &e.handle,
            Self::Audio(e) => &e.handle,
        }
    }
}
