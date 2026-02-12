//! Convenience re-exports.

pub use crate::handler::{
    Handler, FormatHandler,
    PlaintextHandler, CsvHandler, JsonHandler,
    WavHandler, Mp3Handler,
    TextLoader, BinaryLoader, ImageLoader, SpreadsheetLoader, AudioLoader,
};

#[cfg(feature = "html")]
pub use crate::handler::HtmlHandler;
#[cfg(feature = "pdf")]
pub use crate::handler::PdfHandler;
#[cfg(feature = "docx")]
pub use crate::handler::DocxHandler;
#[cfg(feature = "image")]
pub use crate::handler::ImageHandler;
#[cfg(feature = "xlsx")]
pub use crate::handler::XlsxHandler;
#[cfg(feature = "parquet")]
pub use crate::handler::ParquetHandler;

pub use crate::document::Document;
pub use crate::element::{Element, ElementCategory, ElementType};

pub use crate::text::csv::CsvLoader;
pub use crate::text::json::JsonLoader;
pub use crate::text::plaintext::PlaintextLoader;

#[cfg(feature = "html")]
pub use crate::text::html::HtmlLoader;
#[cfg(feature = "pdf")]
pub use crate::binary::pdf::PdfLoader;
#[cfg(feature = "docx")]
pub use crate::binary::docx::DocxLoader;
#[cfg(feature = "image")]
pub use crate::image::image::ImageFileLoader;
#[cfg(feature = "xlsx")]
pub use crate::tabular::xlsx::XlsxLoader;
#[cfg(feature = "parquet")]
pub use crate::tabular::parquet::ParquetLoader;
pub use crate::audio::wav::WavLoader;
pub use crate::audio::mp3::Mp3Loader;
