//! Tabular (spreadsheet) handlers and capability trait.
//!
//! Tabular handlers address content by cell coordinate
//! ([`TabularLocation`] = row + column, optionally with intra-cell
//! byte offsets), distinct from text handlers that address content by
//! byte offset in a serialized stream. CSV decodes into a handler
//! that implements **both** capabilities — the same parsed rows are
//! readable either by byte offset (as text) or by `(row, col)` (as a
//! cell). XLSX decodes into a handler that implements **only** the
//! tabular capability — the underlying ZIP-of-XML structure does not
//! map to flat byte offsets.
//!
//! [`TabularLocation`]: nvisy_ontology::entity::TabularLocation

use nvisy_core::Error;
use nvisy_ontology::entity::TabularLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::TextData;
use crate::transform::{Redactions, TabularRedaction};

mod csv_handler;
mod csv_loader;
mod tabular_handler;
mod xlsx_handler;
mod xlsx_loader;

pub use self::csv_handler::{CsvData, CsvHandler};
pub use self::csv_loader::{CsvLoader, CsvParams};
pub use self::tabular_handler::BoxedTabularHandler;
pub use self::xlsx_handler::XlsxHandler;
pub use self::xlsx_loader::{XlsxLoader, XlsxParams};

/// Capability trait for handlers that expose content by cell coordinate.
///
/// Handlers expose tabular content as a stream of
/// [`TabularLocation`]s identifying individual cells, with explicit
/// `read` calls to fetch a cell's value as [`TextData`], and a
/// `redact` call that applies a batch of [`TabularRedaction`]s
/// grouped by cell.
#[async_trait::async_trait]
pub trait TabularHandler: Handler {
    /// Async stream of [`TabularLocation`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, TabularLocation>;

    /// Read the cell value at the given location as text.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &TabularLocation) -> Option<TextData>;

    /// Apply a batch of redactions grouped by [`TabularLocation`].
    ///
    /// Cell identity is supplied by the [`Redactions`] collection's
    /// keys; each redaction within a cell carries intra-cell byte
    /// offsets that the handler maps onto its own cell value.
    async fn redact(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error>;
}
