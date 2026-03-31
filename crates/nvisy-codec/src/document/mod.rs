//! Unified document representation.

mod span;
mod stream;

use derive_more::{From, IsVariant, TryInto};
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::content::{Content, ContentData, ContentSource};
use nvisy_core::media::{
    AudioFormat, DocumentType, ImageFormat, SpreadsheetFormat, TextFormat, WordFormat,
};

pub use self::span::Span;
pub use self::stream::SpanStream;
use crate::handler::{
    AudioData, AudioHandler, AudioSpanId, BoxedAudioHandler, BoxedImageHandler, BoxedRichHandler,
    BoxedTextHandler, CsvLoader, CsvParams, Handler, HtmlLoader, HtmlParams, ImageData,
    ImageHandler, ImageSpanId, JpegLoader, JpegParams, JsonLoader, JsonParams, Loader,
    MarkdownLoader, MarkdownParams, Mp3Loader, Mp3Params, PngLoader, PngParams, TextData,
    TextHandler, TextSpanId, TiffLoader, TiffParams, TxtLoader, TxtParams, WavLoader, WavParams,
    XlsxLoader, XlsxParams,
};

/// A fully type-erased document that can hold any supported format.
///
/// Groups documents into four modality families:
/// - **Text**: plain text, CSV, JSON, HTML, XLSX
/// - **Image**: PNG, JPEG
/// - **Audio**: WAV, MP3
/// - **Rich**: PDF, DOCX (multi-modal documents with text + images)
///
/// Use [`From`] conversions to wrap concrete handlers or boxed wrappers.
/// Use [`IsVariant`]-generated `is_*` methods or [`TryInto`] to
/// extract the inner handler.
#[derive(From, IsVariant, TryInto)]
pub enum Document {
    Text(BoxedTextHandler),
    Image(BoxedImageHandler),
    Audio(BoxedAudioHandler),
    Rich(BoxedRichHandler),
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Document")
            .field(&self.document_type())
            .finish()
    }
}

impl Document {
    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        match self {
            Self::Text(h) => h.document_type(),
            Self::Image(h) => h.document_type(),
            Self::Audio(h) => h.document_type(),
            Self::Rich(h) => h.document_type(),
        }
    }

    /// Content source identity and lineage.
    pub fn source(&self) -> ContentSource {
        match self {
            Self::Text(h) => h.source(),
            Self::Image(h) => h.source(),
            Self::Audio(h) => h.source(),
            Self::Rich(h) => h.source(),
        }
    }

    /// Encode the document back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Text(h) => h.encode(),
            Self::Image(h) => h.encode(),
            Self::Audio(h) => h.encode(),
            Self::Rich(h) => h.encode(),
        }
    }

    /// Stream text spans from text or rich documents.
    ///
    /// Returns an empty stream for image and audio documents.
    pub async fn text_spans(&self) -> SpanStream<'_, TextSpanId, TextData> {
        match self {
            Self::Text(h) => h.text_spans().await,
            Self::Rich(h) => h.text_spans().await,
            _ => SpanStream::new(futures::stream::empty()),
        }
    }

    /// Stream image spans from image or rich documents.
    ///
    /// Returns an empty stream for text and audio documents.
    pub async fn image_spans(&self) -> SpanStream<'_, ImageSpanId, ImageData> {
        match self {
            Self::Image(h) => h.image_spans().await,
            Self::Rich(h) => h.image_spans().await,
            Self::Text(_) | Self::Audio(_) => SpanStream::new(futures::stream::empty()),
        }
    }

    /// Stream audio spans from audio documents.
    ///
    /// Returns an empty stream for text, image, and rich documents.
    pub async fn audio_spans(&self) -> SpanStream<'_, AudioSpanId, AudioData> {
        match self {
            Self::Audio(h) => h.audio_spans().await,
            _ => SpanStream::new(futures::stream::empty()),
        }
    }

    /// Collect all text spans into a `Vec`.
    pub async fn collect_text_spans(&self) -> Vec<Span<TextSpanId, TextData>> {
        self.text_spans().await.collect().await
    }

    /// Collect all image spans into a `Vec`.
    pub async fn collect_image_spans(&self) -> Vec<Span<ImageSpanId, ImageData>> {
        self.image_spans().await.collect().await
    }

    /// Collect all audio spans into a `Vec`.
    pub async fn collect_audio_spans(&self) -> Vec<Span<AudioSpanId, AudioData>> {
        self.audio_spans().await.collect().await
    }

    /// Apply a batch of text redactions to the document.
    ///
    /// Delegates to [`TextTransform::redact_text`] on the underlying
    /// text or rich handler. Returns `Ok(())` for image and audio
    /// documents (no text to redact).
    ///
    /// [`TextTransform::redact_text`]: crate::transform::TextTransform::redact_text
    pub async fn apply_text_redactions(
        &mut self,
        redactions: &[crate::transform::TextRedaction<TextSpanId>],
    ) -> Result<(), Error> {
        use crate::transform::TextTransform;
        match self {
            Self::Text(h) => h.redact_text(redactions).await,
            Self::Rich(h) => h.redact_text(redactions).await,
            Self::Image(_) | Self::Audio(_) => Ok(()),
        }
    }

    /// Apply a batch of image redactions to the document.
    ///
    /// Delegates to [`ImageTransform::redact_images`] on the underlying
    /// image or rich handler. Returns `Ok(())` for text and audio
    /// documents (no images to redact).
    ///
    /// [`ImageTransform::redact_images`]: crate::transform::ImageTransform::redact_images
    pub async fn apply_image_redactions(
        &mut self,
        redactions: &[crate::transform::ImageRedaction],
    ) -> Result<(), Error> {
        use crate::transform::ImageTransform;
        match self {
            Self::Image(h) => h.redact_images(redactions).await,
            Self::Rich(h) => h.redact_images(redactions).await,
            Self::Text(_) | Self::Audio(_) => Ok(()),
        }
    }

    /// Apply a batch of audio redactions to the document.
    ///
    /// Delegates to [`AudioTransform::redact_audio`] on the underlying
    /// audio handler. Returns `Ok(())` for text, image, and rich
    /// documents (no audio to redact).
    ///
    /// [`AudioTransform::redact_audio`]: crate::transform::AudioTransform::redact_audio
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: &[crate::transform::AudioRedaction],
    ) -> Result<(), Error> {
        use crate::transform::AudioTransform;
        match self {
            Self::Audio(h) => h.redact_audio(redactions).await,
            Self::Text(_) | Self::Image(_) | Self::Rich(_) => Ok(()),
        }
    }

    /// Decode [`Content`] into a `Document` using default parameters.
    ///
    /// Format detection uses [`Content::infer_document_type`], which
    /// evaluates metadata (supplied MIME, detected MIME, filename
    /// extension) with fallback to magic-byte detection on the raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The content type cannot be determined.
    /// - The detected format has no corresponding loader.
    /// - The loader itself fails to decode the content.
    pub async fn decode(content: &Content) -> Result<Self, Error> {
        let doc_type = content.infer_document_type().ok_or_else(|| {
            Error::validation(
                "unable to detect document type from content; \
                 set a MIME type via ContentMetadata::with_content_type",
                "Document::decode",
            )
        })?;
        let data = content.data();

        match doc_type {
            DocumentType::Text(_) | DocumentType::Html | DocumentType::Spreadsheet(_) => {
                Self::decode_text(doc_type, data).await
            }
            DocumentType::Image(_) => Self::decode_image(doc_type, data).await,
            DocumentType::Audio(_) => Self::decode_audio(doc_type, data).await,
            DocumentType::Pdf | DocumentType::Word(_) | DocumentType::Presentation(_) => {
                Self::decode_rich(doc_type, data).await
            }
        }
    }

    async fn decode_text(doc_type: DocumentType, content: &ContentData) -> Result<Self, Error> {
        let handler: BoxedTextHandler = match doc_type {
            DocumentType::Text(TextFormat::Txt | TextFormat::Log) => TxtLoader
                .decode(content, &TxtParams::default())
                .await?
                .into(),
            DocumentType::Text(TextFormat::Json) => JsonLoader
                .decode(content, &JsonParams::default())
                .await?
                .into(),
            DocumentType::Text(TextFormat::Markdown) => MarkdownLoader
                .decode(content, &MarkdownParams::default())
                .await?
                .into(),
            DocumentType::Html => HtmlLoader
                .decode(content, &HtmlParams::default())
                .await?
                .into(),
            DocumentType::Spreadsheet(SpreadsheetFormat::Csv) => CsvLoader
                .decode(content, &CsvParams::default())
                .await?
                .into(),
            DocumentType::Spreadsheet(SpreadsheetFormat::Xlsx) => {
                XlsxLoader.decode(content, &XlsxParams).await?.into()
            }
            _ => {
                return Err(Error::validation(
                    format!("no text loader for: {doc_type}"),
                    "Document::decode_text",
                ));
            }
        };
        Ok(Self::from(handler))
    }

    async fn decode_image(doc_type: DocumentType, content: &ContentData) -> Result<Self, Error> {
        let handler: BoxedImageHandler = match doc_type {
            DocumentType::Image(ImageFormat::Png) => {
                PngLoader.decode(content, &PngParams).await?.into()
            }
            DocumentType::Image(ImageFormat::Jpeg) => {
                JpegLoader.decode(content, &JpegParams).await?.into()
            }
            DocumentType::Image(ImageFormat::Tiff) => {
                TiffLoader.decode(content, &TiffParams).await?.into()
            }
            _ => {
                return Err(Error::validation(
                    format!("no image loader for: {doc_type}"),
                    "Document::decode_image",
                ));
            }
        };
        Ok(Self::from(handler))
    }

    async fn decode_audio(doc_type: DocumentType, content: &ContentData) -> Result<Self, Error> {
        let handler: BoxedAudioHandler = match doc_type {
            DocumentType::Audio(AudioFormat::Wav) => {
                WavLoader.decode(content, &WavParams).await?.into()
            }
            DocumentType::Audio(AudioFormat::Mp3) => {
                Mp3Loader.decode(content, &Mp3Params).await?.into()
            }
            _ => {
                return Err(Error::validation(
                    format!("no audio loader for: {doc_type}"),
                    "Document::decode_audio",
                ));
            }
        };
        Ok(Self::from(handler))
    }

    async fn decode_rich(doc_type: DocumentType, content: &ContentData) -> Result<Self, Error> {
        match doc_type {
            DocumentType::Pdf => {
                #[cfg(feature = "pdf")]
                {
                    use crate::handler::{PdfLoader, PdfParams};
                    let handler = PdfLoader.decode(content, &PdfParams::default()).await?;
                    Ok(Self::from(BoxedRichHandler::from(handler)))
                }
                #[cfg(not(feature = "pdf"))]
                {
                    Err(Error::validation(
                        "PDF support requires the \"pdf\" feature",
                        "Document::decode_rich",
                    ))
                }
            }
            DocumentType::Word(WordFormat::Docx) => {
                #[cfg(feature = "docx")]
                {
                    use crate::handler::{DocxLoader, DocxParams};
                    let handler = DocxLoader.decode(content, &DocxParams).await?;
                    Ok(Self::from(BoxedRichHandler::from(handler)))
                }
                #[cfg(not(feature = "docx"))]
                {
                    Err(Error::validation(
                        "DOCX support requires the \"docx\" feature",
                        "Document::decode_rich",
                    ))
                }
            }
            _ => Err(Error::validation(
                format!("no rich loader for: {doc_type}"),
                "Document::decode_rich",
            )),
        }
    }
}
