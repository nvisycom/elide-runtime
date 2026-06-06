//! Text modality: `impl Codable for Text` plus the concrete format
//! implementations that produce text handles (TXT, JSON, Markdown,
//! HTML). The per-modality capability surface lives on the generic
//! [`Handle<Text>`] trait in [`crate::core`]; replacements written
//! during [`IndexedHandle::redact`] use [`TextReplacement`].
//!
//! [`Handle<Text>`]: crate::core::Handle
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact
//! [`TextReplacement`]: nvisy_core::redaction::TextReplacement

use nvisy_core::modality::{ModalityKind, Text};

use crate::core::Codable;

impl Codable for Text {
    const KIND: ModalityKind = ModalityKind::Text;
}

pub(crate) mod redact;

#[cfg(feature = "html")]
mod html_handler;
#[cfg(feature = "html")]
mod html_loader;
#[cfg(feature = "json")]
mod json_handler;
#[cfg(feature = "json")]
mod json_loader;
#[cfg(feature = "markdown")]
mod markdown_loader;
#[cfg(feature = "txt")]
mod txt_handler;
#[cfg(feature = "txt")]
mod txt_loader;

#[cfg(feature = "html")]
pub use self::html_handler::{HtmlData, HtmlHandler, format as html_format};
#[cfg(feature = "html")]
pub use self::html_loader::HtmlLoader;
#[cfg(feature = "json")]
pub use self::json_handler::{JsonData, JsonHandler, JsonIndent, format as json_format};
#[cfg(feature = "json")]
pub use self::json_loader::JsonLoader;
#[cfg(feature = "markdown")]
pub use self::markdown_loader::{MarkdownLoader, format as markdown_format};
#[cfg(feature = "txt")]
pub use self::txt_handler::{TxtHandler, format as txt_format};
#[cfg(feature = "txt")]
pub use self::txt_loader::TxtLoader;
