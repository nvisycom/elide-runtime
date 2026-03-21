//! Unified document representation.

mod span;
mod stream;

use derive_more::{From, IsVariant, TryInto};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{
    AudioFormat, DocumentType, ImageFormat, SpreadsheetFormat, TextFormat, WordFormat,
};

pub use self::span::Span;
pub use self::stream::SpanStream;
use crate::handler::{
    BoxedAudioHandler, BoxedImageHandler, BoxedRichHandler, BoxedTextHandler, CsvLoader, CsvParams,
    Handler, HtmlLoader, HtmlParams, ImageData, ImageHandler, ImageSpanId, JpegLoader, JpegParams,
    JsonLoader, JsonParams, Loader, Mp3Loader, Mp3Params, PngLoader, PngParams, TextData,
    TextHandler, TextSpanId, TxtLoader, TxtParams, WavLoader, WavParams, XlsxLoader, XlsxParams,
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

    /// Stream image spans from image documents.
    ///
    /// Returns an empty stream for text, audio, and rich documents.
    /// Rich document image support will be added when `RichHandler`
    /// implements `ImageHandler`.
    pub async fn image_spans(&self) -> SpanStream<'_, ImageSpanId, ImageData> {
        match self {
            Self::Image(h) => h.image_spans().await,
            _ => SpanStream::new(futures::stream::empty()),
        }
    }

    /// Decode [`ContentData`] into a `Document` using default parameters.
    ///
    /// Format detection is delegated to
    /// [`ContentData::infer_document_type`], which evaluates three
    /// strategies (supplied MIME, magic bytes, filename extension).
    /// See its documentation for details.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The content type cannot be determined by either strategy.
    /// - The detected format has no corresponding loader.
    /// - The loader itself fails to decode the content.
    pub async fn decode(content: &ContentData) -> Result<Self, Error> {
        let doc_type = content.infer_document_type().ok_or_else(|| {
            Error::validation(
                "unable to detect document type from content; \
                 set a MIME type via ContentData::with_content_type for text formats",
                "Document::decode",
            )
        })?;

        match doc_type {
            // Text formats (require explicit MIME — no magic bytes)
            DocumentType::Text(TextFormat::Txt | TextFormat::Log) => {
                let handler = TxtLoader.decode(content, &TxtParams::default()).await?;
                Ok(Self::from(BoxedTextHandler::from(handler)))
            }
            DocumentType::Text(TextFormat::Json) => {
                let handler = JsonLoader.decode(content, &JsonParams::default()).await?;
                Ok(Self::from(BoxedTextHandler::from(handler)))
            }
            DocumentType::Html => {
                let handler = HtmlLoader.decode(content, &HtmlParams::default()).await?;
                Ok(Self::from(BoxedTextHandler::from(handler)))
            }
            DocumentType::Spreadsheet(SpreadsheetFormat::Csv) => {
                let handler = CsvLoader.decode(content, &CsvParams::default()).await?;
                Ok(Self::from(BoxedTextHandler::from(handler)))
            }
            DocumentType::Spreadsheet(SpreadsheetFormat::Xlsx) => {
                let handler = XlsxLoader.decode(content, &XlsxParams).await?;
                Ok(Self::from(BoxedTextHandler::from(handler)))
            }

            // Image formats (magic-byte detected)
            DocumentType::Image(ImageFormat::Png) => {
                let handler = PngLoader.decode(content, &PngParams).await?;
                Ok(Self::from(BoxedImageHandler::from(handler)))
            }
            DocumentType::Image(ImageFormat::Jpeg) => {
                let handler = JpegLoader.decode(content, &JpegParams).await?;
                Ok(Self::from(BoxedImageHandler::from(handler)))
            }

            // Audio formats (magic-byte detected)
            DocumentType::Audio(AudioFormat::Wav) => {
                let handler = WavLoader.decode(content, &WavParams).await?;
                Ok(Self::from(BoxedAudioHandler::from(handler)))
            }
            DocumentType::Audio(AudioFormat::Mp3) => {
                let handler = Mp3Loader.decode(content, &Mp3Params).await?;
                Ok(Self::from(BoxedAudioHandler::from(handler)))
            }

            // Rich formats (magic-byte detected)
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
                        "Document::decode",
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
                        "Document::decode",
                    ))
                }
            }

            _ => Err(Error::validation(
                format!("no loader available for detected type: {doc_type}"),
                "Document::decode",
            )),
        }
    }
}
