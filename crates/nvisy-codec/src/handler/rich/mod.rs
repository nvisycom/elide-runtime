//! Rich document format handlers and loaders.

#[cfg(feature = "pdf")]
mod pdf_handler;
#[cfg(feature = "pdf")]
mod pdf_loader;
#[cfg(feature = "pdf")]
mod pdf_render;

#[cfg(feature = "docx")]
mod docx_loader;

mod rich_handler;

#[cfg(feature = "docx")]
pub use self::docx_loader::{DocxLoader, DocxParams};
#[cfg(feature = "pdf")]
pub use self::pdf_handler::{RichTextHandler, RichTextSpan};
#[cfg(feature = "pdf")]
pub use self::pdf_loader::{PdfLoader, PdfParams};
pub use self::rich_handler::BoxedRichHandler;
