//! Convenience re-exports.

pub use crate::loaders::csv_loader::CsvLoader;
pub use crate::loaders::json_loader::JsonLoader;
pub use crate::loaders::plaintext::PlaintextLoader;

#[cfg(feature = "pdf")]
pub use crate::loaders::pdf_loader::PdfLoader;
#[cfg(feature = "docx")]
pub use crate::loaders::docx_loader::DocxLoader;
#[cfg(feature = "html")]
pub use crate::loaders::html_loader::HtmlLoader;
#[cfg(feature = "image")]
pub use crate::loaders::image_loader::ImageLoader;
#[cfg(feature = "parquet")]
pub use crate::loaders::parquet_loader::ParquetLoader;
#[cfg(feature = "xlsx")]
pub use crate::loaders::xlsx_loader::XlsxLoader;
pub use crate::loaders::audio_loader::AudioLoader;
