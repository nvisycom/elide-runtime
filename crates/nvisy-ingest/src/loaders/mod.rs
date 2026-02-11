//! File-format loaders for multimodal document ingestion.

use serde::de::DeserializeOwned;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::datatypes::document::ImageData;
use nvisy_core::datatypes::document::TabularData;
use nvisy_core::error::Error;

/// Output of a loader -- either a parsed document, an extracted image, or tabular data.
pub enum LoaderOutput {
    /// A successfully parsed text document.
    Document(Document),
    /// An extracted or decoded image.
    Image(ImageData),
    /// Tabular data extracted from a spreadsheet or data file.
    Tabular(TabularData),
}

/// Converts raw [`Blob`] content into structured [`Document`]s or [`ImageData`].
///
/// Loaders declare which file extensions and MIME types they support.
/// The engine selects the appropriate loader based on the blob's
/// content type and extension.
#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
    /// Strongly-typed parameters for this loader.
    type Params: DeserializeOwned + Send;

    /// Unique identifier for this loader (e.g. `"csv"`, `"pdf"`).
    fn id(&self) -> &str;
    /// File extensions this loader handles (e.g. `["csv", "tsv"]`).
    fn extensions(&self) -> &[&str];
    /// MIME types this loader handles (e.g. `["text/csv"]`).
    fn content_types(&self) -> &[&str];

    /// Parse the blob and return one or more documents or images.
    async fn load(
        &self,
        blob: &Blob,
        params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error>;
}

/// Loader for CSV files.
pub mod csv_loader;
/// Loader for JSON files.
pub mod json_loader;
/// Loader for plain-text files.
pub mod plaintext;

/// Loader for PDF files.
#[cfg(feature = "pdf")]
pub mod pdf_loader;
/// Loader for DOCX (Office Open XML) files.
#[cfg(feature = "docx")]
pub mod docx_loader;
/// Loader for HTML files.
#[cfg(feature = "html")]
pub mod html_loader;
/// Loader for image files (PNG, JPEG, TIFF, etc.).
#[cfg(feature = "image")]
pub mod image_loader;
/// Loader for Apache Parquet files.
#[cfg(feature = "parquet")]
pub mod parquet_loader;
/// Loader for Excel XLSX/XLS files.
#[cfg(feature = "xlsx")]
pub mod xlsx_loader;
/// Placeholder loader for audio files.
pub mod audio_loader;
