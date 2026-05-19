//! [`TabularTransform`]: blanket-impl extension that iterates a
//! [`Redactions`] collection per-cell, dispatching to the handler's
//! [`TabularHandler::redact_at`] hook.

use nvisy_core::Error;
use nvisy_ontology::entity::TabularLocation;

use super::instruction::TabularRedaction;
use crate::handler::TabularHandler;
use crate::transform::Redactions;

/// Apply a batch of tabular redactions, one per cell location.
///
/// Blanket-implemented for every [`TabularHandler`]: the handler only
/// implements the narrow [`TabularHandler::redact_at`] hook;
/// iteration over the [`Redactions`] collection lives here so
/// handlers can't accidentally drop or reorder the location key.
#[async_trait::async_trait]
pub trait TabularTransform: TabularHandler {
    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    async fn redact(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TabularHandler + ?Sized> TabularTransform for H {
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
