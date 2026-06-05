//! Rich-document format implementations: PDF, DOCX.

#[cfg(feature = "docx")]
mod docx_handler;
#[cfg(feature = "docx")]
mod docx_loader;
#[cfg(feature = "pdf")]
mod pdf_handler;
#[cfg(feature = "pdf")]
mod pdf_loader;
#[cfg(feature = "pdf")]
mod pdf_render;

#[cfg(feature = "docx")]
pub use self::docx_handler::{DocxHandler, format as docx_format};
#[cfg(feature = "docx")]
pub use self::docx_loader::DocxLoader;
#[cfg(feature = "pdf")]
pub use self::pdf_handler::{PdfHandler, format as pdf_format};
#[cfg(feature = "pdf")]
pub use self::pdf_loader::PdfLoader;
