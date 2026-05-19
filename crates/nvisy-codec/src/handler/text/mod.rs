//! Text-based format handlers.

use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::transform::{Redactions, TextRedaction};

mod csv_handler;
mod csv_loader;
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
mod xlsx_handler;
mod xlsx_loader;

pub use self::csv_handler::{CsvData, CsvHandler};
pub use self::csv_loader::{CsvLoader, CsvParams};
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
pub use self::xlsx_handler::XlsxHandler;
pub use self::xlsx_loader::{XlsxLoader, XlsxParams};

/// Capability trait for handlers that expose text content.
///
/// Handlers expose text content as a stream of [`TextLocation`]s
/// (cheap, identity-only), with explicit `read` calls to fetch the
/// payload for any given location, and a `redact` call that applies a
/// batch of [`TextRedaction`]s grouped by location.
///
/// # Offset semantics
///
/// Byte offsets in [`TextLocation`] are relative to the handler's
/// **serialized** form. For plain text this is identical to the
/// in-memory form; for JSON and CSV the offsets include formatting
/// characters (quotes, escapes, delimiters). Use [`read`] to extract
/// the logical value at a location rather than slicing the serialized
/// bytes directly.
///
/// [`read`]: TextHandler::read
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Async stream of [`TextLocation`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, TextLocation>;

    /// Read the text content at the given location.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &TextLocation) -> Option<TextData>;

    /// Apply a batch of redactions grouped by [`TextLocation`].
    ///
    /// The collection enforces overlap policy on insert; this method
    /// trusts that ranges within a single location do not overlap.
    async fn redact(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error>;
}
