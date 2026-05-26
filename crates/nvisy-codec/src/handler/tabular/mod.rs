//! Tabular-handler trait + supporting infrastructure.
//!
//! Tabular handlers address content by cell coordinate
//! ([`Tabular`] = row + column, optionally with intra-cell
//! byte offsets), distinct from text handlers that address content by
//! byte offset in a serialized stream.
//!
//! The trait, redaction shape, and `apply_tabular_redaction` helper
//! live here; concrete per-format implementations (CSV, XLSX) live
//! in `nvisy-formats`.
//!
//! [`Tabular`]: nvisy_ontology::modality::Tabular

use nvisy_core::Error;
use nvisy_ontology::modality::Tabular;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::{Redactions, TextData};

mod apply;
mod boxed;
mod instruction;

pub use self::apply::apply_tabular_redaction;
pub use self::boxed::BoxedTabularHandler;
pub use self::instruction::TabularRedaction;

/// Capability trait for handlers that expose content by cell coordinate.
///
/// Handlers implement three narrow operations:
/// - [`locations`]: cheap, identity-only stream of [`Tabular`]s
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
    /// Async stream of [`Tabular`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, Tabular>;

    /// Read the cell value at the given location as text.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &Tabular) -> Option<TextData>;

    /// Apply a single redaction to the cell at `location`, mutating
    /// in place. The cell coordinates and optional intra-cell byte
    /// offsets come from `location`.
    async fn redact_at(
        &mut self,
        location: &Tabular,
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
        redactions: Redactions<Tabular, TabularRedaction>,
    ) -> Result<(), Error> {
        for (location, redaction) in redactions {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
