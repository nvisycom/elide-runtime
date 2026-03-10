//! Text-based format handlers.

use std::hash::Hash;

use futures::StreamExt;
use nvisy_core::Error;

use super::Handler;
use crate::document::{Span, SpanStream};

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
/// scheme via [`TextId`](Self::TextId) (e.g. line numbers for plain
/// text, JSON paths for JSON, page-level IDs for rich documents).
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Strongly-typed identifier for a text span within this handler.
    ///
    /// Must be hashable so that edit routing can map IDs back to their
    /// original spans.
    type TextId: Send + Sync + Clone + Eq + Hash + 'static;

    /// Return text content as an async stream of [`Span`](crate::document::Span)s.
    ///
    /// Each span carries a [`TextId`](Self::TextId) and [`TextData`] payload.
    async fn text_spans(&self) -> SpanStream<'_, Self::TextId, TextData>;

    /// Apply text edits from an async stream back to the handler.
    ///
    /// The stream items must use the same [`TextId`](Self::TextId)
    /// returned by [`text_spans`](Self::text_spans).
    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, Self::TextId, TextData>,
    ) -> Result<(), Error>;
}

/// Re-index a handler's span stream to sequential `usize` IDs.
pub(crate) async fn reindex_stream<'a, H: TextHandler + 'a>(
    handler: &'a H,
) -> SpanStream<'a, usize, TextData> {
    let inner = handler.text_spans().await;
    SpanStream::new(inner.enumerate().map(|(i, s)| Span::new(i, s.data).with_source(s.source)))
}

/// Collect native IDs from a handler, map `usize` edits back, and apply.
pub(crate) async fn forward_edits<H: TextHandler>(
    handler: &mut H,
    edits: Vec<Span<usize, TextData>>,
) -> Result<(), Error> {
    let ids: Vec<H::TextId> = handler.text_spans().await.map(|s| s.id).collect().await;
    let mapped: Vec<_> = edits
        .into_iter()
        .filter_map(|e| ids.get(e.id).cloned().map(|id| Span::new(id, e.data)))
        .collect();
    handler
        .edit_text(SpanStream::new(futures::stream::iter(mapped)))
        .await
}
