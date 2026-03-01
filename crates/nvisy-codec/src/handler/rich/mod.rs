//! Rich document format handlers and loaders.

#[cfg(feature = "pdf")]
mod pdf_handler;
#[cfg(feature = "pdf")]
mod pdf_loader;
#[cfg(feature = "pdf")]
mod pdf_render;

#[cfg(feature = "docx")]
mod docx_handler;
#[cfg(feature = "docx")]
mod docx_loader;

#[cfg(feature = "pdf")]
pub use pdf_handler::{PdfHandler, PdfTextSpan, PdfImageSpan};
#[cfg(feature = "pdf")]
pub use pdf_loader::{PdfLoader, PdfParams};

#[cfg(feature = "docx")]
pub use docx_handler::DocxHandler;
#[cfg(feature = "docx")]
pub use docx_loader::{DocxLoader, DocxParams};
