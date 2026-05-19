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
use crate::handler::Redactions;

mod apply;
mod csv_handler;
mod csv_loader;
mod instruction;
mod tabular_handler;
mod xlsx_handler;
mod xlsx_loader;

pub(crate) use self::apply::apply_tabular_redaction;
pub use self::csv_handler::{CsvData, CsvHandler};
pub use self::instruction::TabularRedaction;
pub use self::csv_loader::{CsvLoader, CsvParams};
pub use self::tabular_handler::BoxedTabularHandler;
pub use self::xlsx_handler::XlsxHandler;
pub use self::xlsx_loader::{XlsxLoader, XlsxParams};

/// Capability trait for handlers that expose content by cell coordinate.
///
/// Handlers implement three narrow operations:
/// - [`locations`]: cheap, identity-only stream of [`TabularLocation`]s
///   identifying individual cells.
/// - [`read`]: fetch a cell's value as [`TextData`].
/// - [`redact_at`]: apply a single redaction to a single cell.
///
/// Batched redaction is provided by [`redact`], which loops
/// [`redact_at`] in insertion order.
///
/// [`locations`]: TabularHandler::locations
/// [`read`]: TabularHandler::read
/// [`redact_at`]: TabularHandler::redact_at
/// [`redact`]: TabularHandler::redact
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

    /// Apply a single redaction to the cell at `location`, mutating
    /// in place. The cell coordinates and optional intra-cell byte
    /// offsets come from `location`.
    async fn redact_at(
        &mut self,
        location: &TabularLocation,
        redaction: TabularRedaction,
    ) -> Result<(), Error>;

    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    ///
    /// The default loops [`redact_at`] in [`Redactions`] insertion
    /// order; handlers with ordering constraints override this
    /// default.
    ///
    /// [`redact_at`]: TabularHandler::redact_at
    async fn redact(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
        for (location, redaction) in redactions {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
