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

mod rich_handler;

#[cfg(feature = "docx")]
pub use docx_handler::DocxHandler;
#[cfg(feature = "docx")]
pub use docx_loader::{DocxLoader, DocxParams};
#[cfg(feature = "pdf")]
pub use pdf_handler::{PdfHandler, PdfImageSpan, PdfTextSpan};
#[cfg(feature = "pdf")]
pub use pdf_loader::{PdfLoader, PdfParams};
pub use rich_handler::AnyRich;
