//! Handler trait, format handler enum, and loader traits.
//!
//! The [`Handler`] supertrait defines metadata shared by all format handlers.
//! The closed [`FormatHandler`] enum provides type erasure so that
//! `Document<FormatHandler>` can represent any supported format in
//! heterogeneous collections.
//!
//! Loader traits ([`TextLoader`], [`BinaryLoader`], [`ImageLoader`],
//! [`SpreadsheetLoader`], [`AudioLoader`]) extend `Handler` with a typed
//! `load()` method that returns `Vec<Document<Self>>`.

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;

// ---------------------------------------------------------------------------
// Handler supertrait
// ---------------------------------------------------------------------------

/// Base trait for all format handlers.
///
/// Every concrete handler (e.g. `CsvHandler`, `PdfHandler`) implements this
/// trait, providing an identifier, supported file extensions, and MIME types.
pub trait Handler: Send + Sync + Clone + 'static {
    /// Unique identifier (e.g. `"csv"`, `"pdf"`, `"wav"`).
    fn id(&self) -> &str;
    /// File extensions this handler supports (e.g. `&["csv"]`).
    fn extensions(&self) -> &[&str];
    /// MIME content types this handler supports (e.g. `&["text/csv"]`).
    fn content_types(&self) -> &[&str];
}

// ---------------------------------------------------------------------------
// Concrete handler structs
// ---------------------------------------------------------------------------

/// Handles plain-text files (`.txt`, `.text`).
#[derive(Debug, Clone)]
pub struct PlaintextHandler;

impl Handler for PlaintextHandler {
    fn id(&self) -> &str { "plaintext" }
    fn extensions(&self) -> &[&str] { &["txt", "text"] }
    fn content_types(&self) -> &[&str] { &["text/plain"] }
}

/// Handles CSV files (`.csv`).
#[derive(Debug, Clone)]
pub struct CsvHandler;

impl Handler for CsvHandler {
    fn id(&self) -> &str { "csv" }
    fn extensions(&self) -> &[&str] { &["csv"] }
    fn content_types(&self) -> &[&str] { &["text/csv"] }
}

/// Handles JSON files (`.json`).
#[derive(Debug, Clone)]
pub struct JsonHandler;

impl Handler for JsonHandler {
    fn id(&self) -> &str { "json" }
    fn extensions(&self) -> &[&str] { &["json"] }
    fn content_types(&self) -> &[&str] { &["application/json"] }
}

/// Handles HTML files (`.html`, `.htm`).
#[cfg(feature = "html")]
#[derive(Debug, Clone)]
pub struct HtmlHandler;

#[cfg(feature = "html")]
impl Handler for HtmlHandler {
    fn id(&self) -> &str { "html" }
    fn extensions(&self) -> &[&str] { &["html", "htm"] }
    fn content_types(&self) -> &[&str] { &["text/html"] }
}

/// Handles PDF files (`.pdf`).
#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfHandler;

#[cfg(feature = "pdf")]
impl Handler for PdfHandler {
    fn id(&self) -> &str { "pdf" }
    fn extensions(&self) -> &[&str] { &["pdf"] }
    fn content_types(&self) -> &[&str] { &["application/pdf"] }
}

/// Handles DOCX files (`.docx`).
#[cfg(feature = "docx")]
#[derive(Debug, Clone)]
pub struct DocxHandler;

#[cfg(feature = "docx")]
impl Handler for DocxHandler {
    fn id(&self) -> &str { "docx" }
    fn extensions(&self) -> &[&str] { &["docx"] }
    fn content_types(&self) -> &[&str] { &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"] }
}

/// Handles image files (PNG, JPEG, TIFF).
#[cfg(feature = "image")]
#[derive(Debug, Clone)]
pub struct ImageHandler;

#[cfg(feature = "image")]
impl Handler for ImageHandler {
    fn id(&self) -> &str { "image" }
    fn extensions(&self) -> &[&str] { &["jpg", "jpeg", "png", "tiff"] }
    fn content_types(&self) -> &[&str] { &["image/jpeg", "image/png", "image/tiff"] }
}

/// Handles XLSX/XLS spreadsheet files.
#[cfg(feature = "xlsx")]
#[derive(Debug, Clone)]
pub struct XlsxHandler;

#[cfg(feature = "xlsx")]
impl Handler for XlsxHandler {
    fn id(&self) -> &str { "xlsx" }
    fn extensions(&self) -> &[&str] { &["xlsx", "xls"] }
    fn content_types(&self) -> &[&str] { &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "application/vnd.ms-excel"] }
}

/// Handles WAV audio files.
#[derive(Debug, Clone)]
pub struct WavHandler;

impl Handler for WavHandler {
    fn id(&self) -> &str { "wav" }
    fn extensions(&self) -> &[&str] { &["wav"] }
    fn content_types(&self) -> &[&str] { &["audio/wav", "audio/x-wav"] }
}

/// Handles MP3 audio files.
#[derive(Debug, Clone)]
pub struct Mp3Handler;

impl Handler for Mp3Handler {
    fn id(&self) -> &str { "mp3" }
    fn extensions(&self) -> &[&str] { &["mp3"] }
    fn content_types(&self) -> &[&str] { &["audio/mpeg"] }
}

// ---------------------------------------------------------------------------
// FormatHandler enum — closed type erasure
// ---------------------------------------------------------------------------

/// Closed enum of all supported format handlers.
///
/// Provides type erasure: `Document<FormatHandler>` can represent
/// content from any supported format in heterogeneous collections.
#[derive(Debug, Clone)]
pub enum FormatHandler {
    Plaintext(PlaintextHandler),
    Csv(CsvHandler),
    Json(JsonHandler),
    #[cfg(feature = "html")]
    Html(HtmlHandler),
    #[cfg(feature = "pdf")]
    Pdf(PdfHandler),
    #[cfg(feature = "docx")]
    Docx(DocxHandler),
    #[cfg(feature = "image")]
    Image(ImageHandler),
    #[cfg(feature = "xlsx")]
    Xlsx(XlsxHandler),
    Wav(WavHandler),
    Mp3(Mp3Handler),
}

impl Handler for FormatHandler {
    fn id(&self) -> &str {
        match self {
            Self::Plaintext(h) => h.id(),
            Self::Csv(h) => h.id(),
            Self::Json(h) => h.id(),
            #[cfg(feature = "html")]
            Self::Html(h) => h.id(),
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => h.id(),
            #[cfg(feature = "docx")]
            Self::Docx(h) => h.id(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.id(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => h.id(),

            Self::Wav(h) => h.id(),
            Self::Mp3(h) => h.id(),
        }
    }

    fn extensions(&self) -> &[&str] {
        match self {
            Self::Plaintext(h) => h.extensions(),
            Self::Csv(h) => h.extensions(),
            Self::Json(h) => h.extensions(),
            #[cfg(feature = "html")]
            Self::Html(h) => h.extensions(),
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => h.extensions(),
            #[cfg(feature = "docx")]
            Self::Docx(h) => h.extensions(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.extensions(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => h.extensions(),

            Self::Wav(h) => h.extensions(),
            Self::Mp3(h) => h.extensions(),
        }
    }

    fn content_types(&self) -> &[&str] {
        match self {
            Self::Plaintext(h) => h.content_types(),
            Self::Csv(h) => h.content_types(),
            Self::Json(h) => h.content_types(),
            #[cfg(feature = "html")]
            Self::Html(h) => h.content_types(),
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => h.content_types(),
            #[cfg(feature = "docx")]
            Self::Docx(h) => h.content_types(),
            #[cfg(feature = "image")]
            Self::Image(h) => h.content_types(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => h.content_types(),

            Self::Wav(h) => h.content_types(),
            Self::Mp3(h) => h.content_types(),
        }
    }
}

// -- From impls for each concrete handler -> FormatHandler --

impl From<PlaintextHandler> for FormatHandler {
    fn from(h: PlaintextHandler) -> Self { Self::Plaintext(h) }
}
impl From<CsvHandler> for FormatHandler {
    fn from(h: CsvHandler) -> Self { Self::Csv(h) }
}
impl From<JsonHandler> for FormatHandler {
    fn from(h: JsonHandler) -> Self { Self::Json(h) }
}
#[cfg(feature = "html")]
impl From<HtmlHandler> for FormatHandler {
    fn from(h: HtmlHandler) -> Self { Self::Html(h) }
}
#[cfg(feature = "pdf")]
impl From<PdfHandler> for FormatHandler {
    fn from(h: PdfHandler) -> Self { Self::Pdf(h) }
}
#[cfg(feature = "docx")]
impl From<DocxHandler> for FormatHandler {
    fn from(h: DocxHandler) -> Self { Self::Docx(h) }
}
#[cfg(feature = "image")]
impl From<ImageHandler> for FormatHandler {
    fn from(h: ImageHandler) -> Self { Self::Image(h) }
}
#[cfg(feature = "xlsx")]
impl From<XlsxHandler> for FormatHandler {
    fn from(h: XlsxHandler) -> Self { Self::Xlsx(h) }
}
impl From<WavHandler> for FormatHandler {
    fn from(h: WavHandler) -> Self { Self::Wav(h) }
}
impl From<Mp3Handler> for FormatHandler {
    fn from(h: Mp3Handler) -> Self { Self::Mp3(h) }
}

// ---------------------------------------------------------------------------
// Loader traits
// ---------------------------------------------------------------------------

/// Loader for text-based formats (plain text, CSV, JSON, HTML).
#[async_trait::async_trait]
pub trait TextLoader: Handler {
    /// Strongly-typed parameters for this loader.
    type Params: Send;

    /// Parse the content into documents.
    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error>;
}

/// Loader for binary document formats (PDF, DOCX) that produce both
/// text documents and extracted images.
#[async_trait::async_trait]
pub trait BinaryLoader: Handler {
    /// Strongly-typed parameters for this loader.
    type Params: Send;

    /// Parse the content into documents (text pages and extracted images).
    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error>;
}

/// Loader for image formats (PNG, JPEG, TIFF, etc.).
#[async_trait::async_trait]
pub trait ImageLoader: Handler {
    /// Strongly-typed parameters for this loader.
    type Params: Send;

    /// Decode the content into image documents.
    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error>;
}

/// Loader for spreadsheet/tabular formats (XLSX).
#[async_trait::async_trait]
pub trait SpreadsheetLoader: Handler {
    /// Strongly-typed parameters for this loader.
    type Params: Send;

    /// Parse the content into tabular documents.
    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error>;
}

/// Loader for audio formats (WAV, MP3).
#[async_trait::async_trait]
pub trait AudioLoader: Handler {
    /// Strongly-typed parameters for this loader.
    type Params: Send;

    /// Process the audio content.
    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error>;
}
