//! Text-based format handlers.

use std::hash::Hash;

use nvisy_core::Error;

use super::{Handler, SpanEditStream, SpanStream};

mod csv_handler;
mod csv_loader;
#[cfg(feature = "html")]
mod html_handler;
#[cfg(feature = "html")]
mod html_loader;
mod json_handler;
mod json_loader;
mod text_data;
mod text_handler;
mod txt_handler;
mod txt_loader;
#[cfg(feature = "xlsx")]
mod xlsx_handler;
#[cfg(feature = "xlsx")]
mod xlsx_loader;

pub use csv_handler::{CsvData, CsvHandler, CsvSpan};
pub use csv_loader::{CsvLoader, CsvParams};
#[cfg(feature = "html")]
pub use html_handler::{HtmlData, HtmlHandler, HtmlSpan};
#[cfg(feature = "html")]
pub use html_loader::{HtmlLoader, HtmlParams};
pub use json_handler::{JsonData, JsonHandler, JsonIndent, JsonPath};
pub use json_loader::{JsonLoader, JsonParams};
pub use text_data::TextData;
pub use text_handler::AnyText;
pub use txt_handler::{TxtHandler, TxtSpan};
pub use txt_loader::{TxtLoader, TxtParams};
#[cfg(feature = "xlsx")]
pub use xlsx_handler::XlsxHandler;
#[cfg(feature = "xlsx")]
pub use xlsx_loader::{XlsxLoader, XlsxParams};

/// Capability trait for handlers that expose text content.
///
/// Handlers implementing this trait can yield text spans and accept
/// text edits. Each handler defines its own text span addressing
/// scheme via [`TextId`](Self::TextId).
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Strongly-typed identifier for a text span within this handler.
    type TextId: Send + Sync + Clone + Eq + Hash + 'static;

    /// Return text content as an async stream of spans.
    async fn text_spans(&self) -> SpanStream<'_, Self::TextId, TextData>;

    /// Apply text edits from an async stream back to the source structure.
    async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, Self::TextId, TextData>,
    ) -> Result<(), Error>;
}
