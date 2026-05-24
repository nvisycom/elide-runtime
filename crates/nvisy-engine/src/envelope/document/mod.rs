//! Engine-level document: content handle + metadata + artifacts.
//!
//! [`Document`] is the pipeline's view of a file. It wraps the codec's
//! [`ContentHandle`] (encoding/decoding) with [`ContentMetadata`]
//! (MIME type, filename) and [`ContentArtifacts`] (intermediate
//! processing data like OCR results or transcriptions).
//!
//! Modality-specific accessors live in sibling modules
//! (`text`, `tabular`, `image`, `audio`), each gated by the matching
//! feature. This module only contains the struct, the constructor,
//! and operations that are meaningful regardless of modality.

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "image")]
mod image;
#[cfg(feature = "tabular")]
mod tabular;
mod text;

use std::fmt;

use nvisy_codec::ContentHandle;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::artifacts::ContentArtifacts;
use nvisy_ontology::entity::Location;

/// Engine-level document combining content, metadata, and artifacts.
///
/// Created during import and carried through the entire pipeline via
/// [`DocumentEnvelope`].
///
/// [`DocumentEnvelope`]: crate::envelope::DocumentEnvelope
pub struct Document {
    /// The decoded content handle (text, image, audio, or rich).
    pub handle: ContentHandle,
    /// Content metadata (MIME type, filename, etc.) from the original upload.
    pub metadata: ContentMetadata,
    /// Modality-specific processing artifacts (OCR, transcription, etc.).
    pub artifacts: ContentArtifacts,
}

impl Document {
    /// Create a new document from a content handle and metadata.
    pub fn new(handle: ContentHandle, metadata: ContentMetadata) -> Self {
        let artifacts = match &handle {
            ContentHandle::Text(_) => ContentArtifacts::text(),
            #[cfg(feature = "tabular")]
            ContentHandle::Tabular(_) => ContentArtifacts::tabular(),
            #[cfg(feature = "image")]
            ContentHandle::Image(_) => ContentArtifacts::image(),
            #[cfg(feature = "audio")]
            ContentHandle::Audio(_) => ContentArtifacts::audio(),
            #[cfg(feature = "rich")]
            ContentHandle::Rich(_) => ContentArtifacts::rich(),
        };
        Self {
            handle,
            metadata,
            artifacts,
        }
    }

    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        self.handle.document_type()
    }

    /// Content source identity and lineage.
    pub fn source(&self) -> ContentSource {
        self.handle.source()
    }

    /// Encode the document back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        self.handle.encode()
    }

    /// Resolve a [`Location`] to its text representation, dispatching by modality.
    ///
    /// - Text and tabular locations: read from the underlying handler.
    /// - Audio locations: read the transcription text from artifacts.
    /// - Image locations: not yet implemented (OCR results are multi-region).
    pub async fn value_at(&self, location: &Location) -> Option<String> {
        use nvisy_codec::handler::TextData;

        match location {
            Location::Text(loc) => self.read_text(loc).await.map(TextData::into_inner),
            #[cfg(feature = "tabular")]
            Location::Tabular(loc) => self.read_tabular(loc).await.map(TextData::into_inner),
            #[cfg(feature = "audio")]
            Location::Audio(_) => self
                .artifacts
                .as_audio()
                .and_then(|a| a.transcription.as_ref())
                .map(|t| t.text()),
            // Image OCR results are multi-region; per-location lookup
            // is not yet implemented.
            #[cfg(feature = "image")]
            Location::Image(_) => None,
            _ => None,
        }
    }
}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("type", &self.document_type())
            .field("source", &self.source())
            .finish()
    }
}

#[cfg(test)]
impl Document {
    /// Create a test document from plain text content.
    pub(crate) async fn from_text(text: &str) -> Self {
        let data = ContentData::from_text(ContentSource::new(), text);
        let meta = ContentMetadata::new().with_content_type("text/plain");
        let content = nvisy_core::content::Content::with_metadata(data, meta.clone());
        let handle = nvisy_formats::decode(&content).await.expect("decode text");
        Self::new(handle, meta)
    }
}
