//! Text modality: `impl Codable for Text` plus the concrete format
//! implementations that produce text handles (TXT, JSON, Markdown,
//! HTML). The per-modality capability surface lives on the generic
//! [`Handle<Text>`] trait in [`crate::core`]; replacements written
//! during [`Handle::redact`] use [`TextReplacement`].
//!
//! [`Handle<Text>`]: crate::core::Handle
//! [`Handle::redact`]: crate::core::Handle::redact
//! [`TextReplacement`]: nvisy_core::redaction::TextReplacement

use std::ops::Range;

use nvisy_core::modality::{Text, TextLocation};

use crate::core::{Chunk, Codable, ModalityKind};

impl Codable for Text {
    const KIND: ModalityKind = ModalityKind::Text;
}

/// Identity lift for handlers where `chunk.data` is byte-for-byte
/// a slice of the source covered by `chunk.location` — no
/// escapes, no decoding. The default implementation backing
/// [`Handle::lift_chunk`] for TXT lines, HTML text nodes,
/// PDF page text, and DOCX text runs.
///
/// Adds `value_range` to `chunk.location.start` and copies
/// `page_number` / `context` through. Returns `None` when the
/// range falls outside the chunk's value.
///
/// [`Handle::lift_chunk`]: crate::core::Handle::lift_chunk
pub fn lift_identity(chunk: &Chunk<Text>, value_range: Range<usize>) -> Option<TextLocation> {
    let len = chunk.location.end.checked_sub(chunk.location.start)?;
    if value_range.start > value_range.end || value_range.end > len {
        return None;
    }
    Some(TextLocation {
        start: chunk.location.start + value_range.start,
        end: chunk.location.start + value_range.end,
        context: chunk.location.context,
        page_number: chunk.location.page_number,
    })
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
pub use self::json_handler::{JsonHandler, format as json_format};
#[cfg(feature = "json")]
pub use self::json_loader::JsonLoader;
#[cfg(feature = "markdown")]
pub use self::markdown_loader::{MarkdownLoader, format as markdown_format};
#[cfg(feature = "txt")]
pub use self::txt_handler::{TxtHandler, format as txt_format};
#[cfg(feature = "txt")]
pub use self::txt_loader::TxtLoader;
