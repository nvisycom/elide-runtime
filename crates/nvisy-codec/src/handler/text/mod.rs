//! Text modality: concrete format implementations that produce
//! text handles (TXT, JSON, Markdown, HTML). The per-modality
//! capability surface lives on the generic [`Handler<Text>`] trait
//! re-exported at the crate root; replacements written during
//! [`Handler::redact`] use [`TextReplacement`].
//!
//! [`Handler<Text>`]: crate::Handler
//! [`Handler::redact`]: crate::Handler::redact
//! [`TextReplacement`]: nvisy_core::redaction::TextReplacement

pub(crate) mod redact;

#[cfg(feature = "html")]
mod html_encode;
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
pub use self::html_handler::{
    ElementTarget, HtmlData, HtmlHandler, RedactableItem, RedactableKind, format as html_format,
};
#[cfg(feature = "html")]
pub use self::html_loader::{HtmlLoader, ScriptPolicy};
#[cfg(feature = "json")]
pub use self::json_handler::{JsonHandler, format as json_format};
#[cfg(feature = "json")]
pub use self::json_loader::JsonLoader;
#[cfg(feature = "markdown")]
pub use self::markdown_loader::{MdLoader, format as markdown_format};
#[cfg(feature = "txt")]
pub use self::txt_handler::{TxtHandler, format as txt_format};
#[cfg(feature = "txt")]
pub use self::txt_loader::TxtLoader;
