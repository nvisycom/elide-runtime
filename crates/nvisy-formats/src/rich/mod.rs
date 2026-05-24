//! Rich-document format implementations: PDF, DOCX.

#[cfg(feature = "docx")]
mod docx_loader;
#[cfg(feature = "pdf")]
mod pdf_handler;
#[cfg(feature = "pdf")]
mod pdf_loader;
#[cfg(feature = "pdf")]
mod pdf_render;

#[cfg(feature = "docx")]
pub use self::docx_loader::{DocxLoader, DocxParams};
#[cfg(feature = "pdf")]
pub use self::pdf_handler::RichTextHandler;
#[cfg(feature = "pdf")]
pub use self::pdf_loader::{PdfLoader, PdfParams};
