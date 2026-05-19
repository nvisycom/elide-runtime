//! Engine-level document: content handle + metadata + artifacts.
//!
//! [`Document`] is the pipeline's view of a file. It wraps the codec's
//! [`ContentHandle`] (encoding/decoding) with [`ContentMetadata`]
//! (MIME type, filename) and [`ContentArtifacts`] (intermediate
//! processing data like OCR results or transcriptions).

use std::fmt;

use futures::StreamExt;
use nvisy_codec::handler::{AudioData, ImageData, TextData};
use nvisy_codec::handler::{
    AudioRedaction, ImageRedaction, Redactions, TabularRedaction, TextRedaction,
};
use nvisy_codec::{ContentHandle, Located};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::artifacts::ContentArtifacts;
use nvisy_ontology::entity::{
    AudioLocation, ImageLocation, Location, TabularLocation, TextLocation,
};

/// Engine-level document combining content, metadata, and artifacts.
///
/// Created during import and carried through the entire pipeline via
/// [`DocumentEnvelope`].
///
/// [`DocumentEnvelope`]: crate::operation::DocumentEnvelope
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
            ContentHandle::Tabular(_) => ContentArtifacts::tabular(),
            ContentHandle::Image(_) => ContentArtifacts::image(),
            ContentHandle::Audio(_) => ContentArtifacts::audio(),
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

    /// Collect all text locations into a `Vec`.
    pub async fn collect_text_locations(&self) -> Vec<Located<TextLocation>> {
        self.handle.text_locations().collect().await
    }

    /// Collect all tabular (cell) locations into a `Vec`.
    pub async fn collect_tabular_locations(&self) -> Vec<Located<TabularLocation>> {
        self.handle.tabular_locations().collect().await
    }

    /// Collect all image locations into a `Vec`.
    pub async fn collect_image_locations(&self) -> Vec<Located<ImageLocation>> {
        self.handle.image_locations().collect().await
    }

    /// Collect all audio locations into a `Vec`.
    pub async fn collect_audio_locations(&self) -> Vec<Located<AudioLocation>> {
        self.handle.audio_locations().collect().await
    }

    /// Read the text content at the given text location.
    pub async fn read_text(&self, location: &TextLocation) -> Option<TextData> {
        self.handle.read_text(location).await
    }

    /// Read the cell value at the given tabular location.
    pub async fn read_tabular(&self, location: &TabularLocation) -> Option<TextData> {
        self.handle.read_tabular(location).await
    }

    /// Read the image data at the given image location.
    pub async fn read_image(&self, location: &ImageLocation) -> Option<ImageData> {
        self.handle.read_image(location).await
    }

    /// Read the audio data at the given audio location.
    pub async fn read_audio(&self, location: &AudioLocation) -> Option<AudioData> {
        self.handle.read_audio(location).await
    }

    /// Resolve a [`Location`] to its text representation, dispatching by modality.
    ///
    /// - Text and tabular locations: read from the underlying handler.
    /// - Audio locations: read the transcription text from artifacts.
    /// - Image locations: not yet implemented (OCR results are multi-region).
    pub async fn value_at(&self, location: &Location) -> Option<String> {
        match location {
            Location::Text(loc) => self.read_text(loc).await.map(TextData::into_inner),
            Location::Tabular(loc) => self.read_tabular(loc).await.map(TextData::into_inner),
            Location::Audio(_) => self
                .artifacts
                .as_audio()
                .and_then(|a| a.transcription.as_ref())
                .map(|t| t.text()),
            // Image OCR results are multi-region; per-location lookup
            // is not yet implemented.
            Location::Image(_) => None,
            _ => None,
        }
    }

    /// Apply a batch of text redactions to the document.
    pub async fn apply_text_redactions(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_text_redactions(redactions).await
    }

    /// Apply a batch of tabular redactions to the document.
    pub async fn apply_tabular_redactions(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_tabular_redactions(redactions).await
    }

    /// Apply a batch of image redactions to the document.
    pub async fn apply_image_redactions(
        &mut self,
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_image_redactions(redactions).await
    }

    /// Apply a batch of audio redactions to the document.
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_audio_redactions(redactions).await
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
        let handle = ContentHandle::decode(&content).await.expect("decode text");
        Self::new(handle, meta)
    }
}
