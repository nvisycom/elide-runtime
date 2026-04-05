//! Text-based format handlers.

use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::Handler;
use crate::document::SpanStream;

#[cfg(feature = "html")]
mod html_handler;
#[cfg(feature = "html")]
mod html_loader;
mod json_handler;
mod json_loader;
mod markdown_loader;
mod text_data;
mod text_handler;
mod txt_handler;
mod txt_loader;

#[cfg(feature = "html")]
pub use self::html_handler::{HtmlData, HtmlHandler};
#[cfg(feature = "html")]
pub use self::html_loader::{HtmlLoader, HtmlParams};
pub use self::json_handler::{JsonData, JsonHandler, JsonIndent};
pub use self::json_loader::{JsonLoader, JsonParams};
pub use self::markdown_loader::{MarkdownLoader, MarkdownParams};
pub use self::text_data::TextData;
pub use self::text_handler::BoxedTextHandler;
pub use self::txt_handler::TxtHandler;
pub use self::txt_loader::{TxtLoader, TxtParams};

/// Capability trait for handlers that expose text content.
///
/// Handlers implementing this trait yield text spans addressed by
/// [`TextLocation`] and accept text edits keyed by the same type.
///
/// # Offset semantics
///
/// Byte offsets in [`TextLocation`] are relative to the handler's
/// **serialized** form. For plain text this is identical to the
/// in-memory form; for JSON and CSV the offsets include formatting
/// characters (quotes, escapes, delimiters). Use [`value_at`] to
/// extract the logical value at a location rather than slicing the
/// serialized bytes directly.
///
/// [`value_at`]: TextHandler::value_at
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Return text content as an async stream of [`Span`]s.
    ///
    /// Each span carries a [`TextLocation`] identifying its position
    /// within the document and a [`TextData`] payload.
    ///
    /// [`Span`]: crate::document::Span
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData>;

    /// Apply text edits from an async stream back to the handler.
    ///
    /// The stream items must use [`TextLocation`] values that
    /// correspond to spans returned by [`text_spans`](Self::text_spans).
    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error>;

    /// Extract the text value at the given location, if available.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn value_at(&self, location: &TextLocation) -> Option<String>;
}
