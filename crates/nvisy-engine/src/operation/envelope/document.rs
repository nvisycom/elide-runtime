//! Engine-level document: content handle + metadata + artifacts.
//!
//! [`Document`] is the pipeline's view of a file. It wraps the codec's
//! [`ContentHandle`] (encoding/decoding) with [`ContentMetadata`]
//! (MIME type, filename) and [`ContentArtifacts`] (intermediate
//! processing data like OCR results or transcriptions).

use nvisy_codec::handler::{AudioData, ImageData, TextData};
use nvisy_codec::{ContentHandle, Span, SpanStream};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::artifacts::ContentArtifacts;
use nvisy_ontology::entity::{
    AudioLocation, ImageLocation, Location, TabularLocation, TextLocation,
};

use nvisy_codec::transform::{AudioRedaction, ImageRedaction, TabularRedaction, TextRedaction};

/// Engine-level document combining content, metadata, and artifacts.
///
/// Created during import and carried through the entire pipeline via
/// [`DocumentEnvelope`](crate::operation::DocumentEnvelope).
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
    ///
    /// Artifacts are initialized to the appropriate empty variant based
    /// on the handle's modality.
    pub fn new(handle: ContentHandle, metadata: ContentMetadata) -> Self {
        let artifacts = match &handle {
            ContentHandle::Text(_) => ContentArtifacts::text(),
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

    // -- Delegated handle methods --

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

    /// Stream text spans from text or rich documents.
    pub async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        self.handle.text_spans().await
    }

    /// Stream image spans from image or rich documents.
    pub async fn image_spans(&self) -> SpanStream<'_, ImageLocation, ImageData> {
        self.handle.image_spans().await
    }

    /// Stream audio spans from audio documents.
    pub async fn audio_spans(&self) -> SpanStream<'_, AudioLocation, AudioData> {
        self.handle.audio_spans().await
    }

    /// Collect all text spans into a `Vec`.
    pub async fn collect_text_spans(&self) -> Vec<Span<TextLocation, TextData>> {
        self.handle.collect_text_spans().await
    }

    /// Collect all image spans into a `Vec`.
    pub async fn collect_image_spans(&self) -> Vec<Span<ImageLocation, ImageData>> {
        self.handle.collect_image_spans().await
    }

    /// Collect all audio spans into a `Vec`.
    pub async fn collect_audio_spans(&self) -> Vec<Span<AudioLocation, AudioData>> {
        self.handle.collect_audio_spans().await
    }

    // -- Type-safe value access --

    /// Extract the text value at the given text location.
    pub async fn text_at(&self, location: &TextLocation) -> Option<String> {
        use nvisy_codec::handler::TextHandler;
        match &self.handle {
            ContentHandle::Text(h) => h.value_at(location).await,
            ContentHandle::Rich(h) => TextHandler::value_at(h, location).await,
            _ => None,
        }
    }

    /// Extract the image data at the given image location.
    pub async fn image_at(&self, location: &ImageLocation) -> Option<ImageData> {
        use nvisy_codec::handler::ImageHandler;
        match &self.handle {
            ContentHandle::Image(h) => h.value_at(location).await,
            ContentHandle::Rich(h) => ImageHandler::value_at(h, location).await,
            _ => None,
        }
    }

    /// Extract the audio data at the given audio location.
    pub async fn audio_at(&self, location: &AudioLocation) -> Option<AudioData> {
        use nvisy_codec::handler::AudioHandler;
        match &self.handle {
            ContentHandle::Audio(h) => h.value_at(location).await,
            _ => None,
        }
    }

    /// Extract the cell value at the given tabular location.
    ///
    /// Currently returns `None` — tabular documents are handled as text
    /// internally and don't have a dedicated `ContentHandle` variant yet.
    pub async fn tabular_at(&self, _location: &TabularLocation) -> Option<String> {
        None
    }

    /// Extract the text value at a [`Location`], dispatching by modality.
    ///
    /// Returns the text content for Text/Tabular locations and the
    /// extracted text (OCR/STT) for Image/Audio locations.
    pub async fn value_at(&self, location: &Location) -> Option<String> {
        match location {
            Location::Text(loc) => self.text_at(loc).await,
            Location::Image(loc) => loc.extracted_text.clone(),
            Location::Audio(loc) => loc.extracted_text.clone(),
            Location::Tabular(loc) => self.tabular_at(loc).await,
            _ => None,
        }
    }

    // -- Redaction delegates --

    /// Apply a batch of text redactions to the document.
    pub async fn apply_text_redactions(
        &mut self,
        redactions: &[TextRedaction<TextLocation>],
    ) -> Result<(), Error> {
        self.handle.apply_text_redactions(redactions).await
    }

    /// Apply a batch of image redactions to the document.
    pub async fn apply_image_redactions(
        &mut self,
        redactions: &[ImageRedaction<ImageLocation>],
    ) -> Result<(), Error> {
        self.handle.apply_image_redactions(redactions).await
    }

    /// Apply a batch of audio redactions to the document.
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: &[AudioRedaction<AudioLocation>],
    ) -> Result<(), Error> {
        self.handle.apply_audio_redactions(redactions).await
    }

    /// Apply a batch of tabular redactions to the document.
    pub async fn apply_tabular_redactions(
        &mut self,
        redactions: &[TabularRedaction],
    ) -> Result<(), Error> {
        use nvisy_codec::transform::TabularTransform;
        match &mut self.handle {
            ContentHandle::Text(h) => h.redact_tabular(redactions).await,
            _ => Ok(()),
        }
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("type", &self.document_type())
            .field("source", &self.source())
            .finish()
    }
}

