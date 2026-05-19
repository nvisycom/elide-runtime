//! Type-erased content handle for all supported formats.

mod located;
mod span;
mod stream;

use std::fmt;

use derive_more::{From, IsVariant, TryInto};
use nvisy_core::Error;
use nvisy_core::content::{Content, ContentData, ContentSource};
use nvisy_core::media::{
    AudioFormat, DocumentType, ImageFormat, SpreadsheetFormat, TextFormat, WordFormat,
};
use nvisy_ontology::entity::{AudioLocation, ImageLocation, TabularLocation, TextLocation};

pub use self::located::Located;
pub use self::span::Span;
pub use self::stream::LocationStream;
use crate::handler::{
    AudioData, AudioHandler, BoxedAudioHandler, BoxedImageHandler, BoxedRichHandler,
    BoxedTabularHandler, BoxedTextHandler, CsvLoader, CsvParams, Handler, ImageData, ImageHandler,
    JpegLoader, JpegParams, JsonLoader, JsonParams, Loader, MarkdownLoader, MarkdownParams,
    Mp3Loader, Mp3Params, PngLoader, PngParams, TabularHandler, TextData, TextHandler, TiffLoader,
    TiffParams, TxtLoader, TxtParams, WavLoader, WavParams, XlsxLoader, XlsxParams,
};
#[cfg(feature = "docx")]
use crate::handler::{DocxLoader, DocxParams};
#[cfg(feature = "html")]
use crate::handler::{HtmlLoader, HtmlParams};
#[cfg(feature = "pdf")]
use crate::handler::{PdfLoader, PdfParams};
use crate::transform::{
    AudioRedaction, ImageRedaction, Redactions, TabularRedaction, TextRedaction,
};

/// A fully type-erased document that can hold any supported format.
///
/// Groups documents into five modality families:
/// - **Text**: plain text, JSON, HTML
/// - **Tabular**: CSV, XLSX (cell-coordinate addressed)
/// - **Image**: PNG, JPEG, TIFF
/// - **Audio**: WAV, MP3
/// - **Rich**: PDF, DOCX (multi-modal documents with text + images)
#[derive(From, IsVariant, TryInto)]
pub enum ContentHandle {
    Text(BoxedTextHandler),
    Tabular(BoxedTabularHandler),
    Image(BoxedImageHandler),
    Audio(BoxedAudioHandler),
    Rich(BoxedRichHandler),
}

impl fmt::Debug for ContentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentHandle")
            .field(&self.document_type())
            .finish()
    }
}

impl ContentHandle {
    /// The document type of the underlying content.
    pub fn document_type(&self) -> DocumentType {
        match self {
            Self::Text(h) => h.document_type(),
            Self::Tabular(h) => h.document_type(),
            Self::Image(h) => h.document_type(),
            Self::Audio(h) => h.document_type(),
            Self::Rich(h) => h.document_type(),
        }
    }

    /// Content source identity and lineage.
    pub fn source(&self) -> ContentSource {
        match self {
            Self::Text(h) => h.source(),
            Self::Tabular(h) => h.source(),
            Self::Image(h) => h.source(),
            Self::Audio(h) => h.source(),
            Self::Rich(h) => h.source(),
        }
    }

    /// Encode the document back to raw bytes.
    pub fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Text(h) => h.encode(),
            Self::Tabular(h) => h.encode(),
            Self::Image(h) => h.encode(),
            Self::Audio(h) => h.encode(),
            Self::Rich(h) => h.encode(),
        }
    }

    /// Stream text locations from text or rich documents.
    pub fn text_locations(&self) -> LocationStream<'_, TextLocation> {
        match self {
            Self::Text(h) => h.locations(),
            Self::Rich(h) => TextHandler::locations(h),
            Self::Tabular(_) | Self::Image(_) | Self::Audio(_) => LocationStream::empty(),
        }
    }

    /// Stream tabular (cell) locations from spreadsheet documents.
    pub fn tabular_locations(&self) -> LocationStream<'_, TabularLocation> {
        match self {
            Self::Tabular(h) => h.locations(),
            _ => LocationStream::empty(),
        }
    }

    /// Stream image locations from image or rich documents.
    pub fn image_locations(&self) -> LocationStream<'_, ImageLocation> {
        match self {
            Self::Image(h) => h.locations(),
            Self::Rich(h) => ImageHandler::locations(h),
            Self::Text(_) | Self::Tabular(_) | Self::Audio(_) => LocationStream::empty(),
        }
    }

    /// Stream audio locations from audio documents.
    pub fn audio_locations(&self) -> LocationStream<'_, AudioLocation> {
        match self {
            Self::Audio(h) => h.locations(),
            _ => LocationStream::empty(),
        }
    }

    /// Read text data at the given location.
    ///
    /// Returns `None` if the location is out of bounds or the handle
    /// does not expose text content.
    pub async fn read_text(&self, location: &TextLocation) -> Option<TextData> {
        match self {
            Self::Text(h) => h.read(location).await,
            Self::Rich(h) => TextHandler::read(h, location).await,
            Self::Tabular(_) | Self::Image(_) | Self::Audio(_) => None,
        }
    }

    /// Read the cell value at the given tabular location.
    pub async fn read_tabular(&self, location: &TabularLocation) -> Option<TextData> {
        match self {
            Self::Tabular(h) => h.read(location).await,
            _ => None,
        }
    }

    /// Read image data at the given location.
    pub async fn read_image(&self, location: &ImageLocation) -> Option<ImageData> {
        match self {
            Self::Image(h) => h.read(location).await,
            Self::Rich(h) => ImageHandler::read(h, location).await,
            Self::Text(_) | Self::Tabular(_) | Self::Audio(_) => None,
        }
    }

    /// Read audio data at the given location.
    pub async fn read_audio(&self, location: &AudioLocation) -> Option<AudioData> {
        match self {
            Self::Audio(h) => h.read(location).await,
            _ => None,
        }
    }

    /// Apply a batch of text redactions to the document.
    pub async fn apply_text_redactions(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Text(h) => h.redact(redactions).await,
            Self::Rich(h) => TextHandler::redact(h, redactions).await,
            Self::Tabular(_) | Self::Image(_) | Self::Audio(_) => Ok(()),
        }
    }

    /// Apply a batch of tabular redactions to the document.
    pub async fn apply_tabular_redactions(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Tabular(h) => h.redact(redactions).await,
            _ => Ok(()),
        }
    }

    /// Apply a batch of image redactions to the document.
    pub async fn apply_image_redactions(
        &mut self,
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Image(h) => h.redact(redactions).await,
            Self::Rich(h) => ImageHandler::redact(h, redactions).await,
            Self::Text(_) | Self::Tabular(_) | Self::Audio(_) => Ok(()),
        }
    }

    /// Apply a batch of audio redactions to the document.
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error> {
        match self {
            Self::Audio(h) => h.redact(redactions).await,
            _ => Ok(()),
        }
    }

    /// Decode [`Content`] into a `ContentHandle` using default parameters.
    pub async fn decode(content: &Content) -> Result<Self, Error> {
        let doc_type = content.infer_document_type().ok_or_else(|| {
            Error::validation(
                "unable to detect document type from content; \
                 set a MIME type via ContentMetadata::with_content_type",
                "ContentHandle::decode",
            )
        })?;
        let data = content.data();

        match doc_type {
            DocumentType::Text(_) | DocumentType::Html => Self::decode_text(doc_type, data).await,
            DocumentType::Spreadsheet(_) => Self::decode_tabular(doc_type, data).await,
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
            #[cfg(feature = "html")]
            DocumentType::Html => HtmlLoader
                .decode(content, &HtmlParams::default())
                .await?
                .into(),
            _ => {
                return Err(Error::validation(
                    format!("no text loader for: {doc_type}"),
                    "ContentHandle::decode_text",
                ));
            }
        };
        Ok(Self::from(handler))
    }

    async fn decode_tabular(doc_type: DocumentType, content: &ContentData) -> Result<Self, Error> {
        let handler: BoxedTabularHandler = match doc_type {
            DocumentType::Spreadsheet(SpreadsheetFormat::Csv) => CsvLoader
                .decode(content, &CsvParams::default())
                .await?
                .into(),
            #[cfg(feature = "xlsx")]
            DocumentType::Spreadsheet(SpreadsheetFormat::Xlsx) => {
                XlsxLoader.decode(content, &XlsxParams).await?.into()
            }
            _ => {
                return Err(Error::validation(
                    format!("no tabular loader for: {doc_type}"),
                    "ContentHandle::decode_tabular",
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
                    "ContentHandle::decode_image",
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
                    "ContentHandle::decode_audio",
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
                    let handler = PdfLoader.decode(content, &PdfParams::default()).await?;
                    Ok(Self::from(BoxedRichHandler::from(handler)))
                }
                #[cfg(not(feature = "pdf"))]
                {
                    Err(Error::validation(
                        "PDF support requires the \"pdf\" feature",
                        "ContentHandle::decode_rich",
                    ))
                }
            }
            DocumentType::Word(WordFormat::Docx) => {
                #[cfg(feature = "docx")]
                {
                    let handler = DocxLoader.decode(content, &DocxParams).await?;
                    Ok(Self::from(BoxedRichHandler::from(handler)))
                }
                #[cfg(not(feature = "docx"))]
                {
                    Err(Error::validation(
                        "DOCX support requires the \"docx\" feature",
                        "ContentHandle::decode_rich",
                    ))
                }
            }
            _ => Err(Error::validation(
                format!("no rich loader for: {doc_type}"),
                "ContentHandle::decode_rich",
            )),
        }
    }
}
