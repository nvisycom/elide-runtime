//! [`TextTransform`]: blanket-impl extension that iterates a
//! [`Redactions`] collection per-location, dispatching to the
//! handler's [`TextHandler::redact_at`] hook.

use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::instruction::TextRedaction;
use crate::handler::TextHandler;
use crate::transform::Redactions;

/// Apply a batch of text redactions, one per location.
///
/// Blanket-implemented for every [`TextHandler`]: the handler only
/// implements the narrow [`TextHandler::redact_at`] hook; iteration
/// over the [`Redactions`] collection lives here so handlers can't
/// accidentally drop or reorder the location key.
#[async_trait::async_trait]
pub trait TextTransform: TextHandler {
    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    async fn redact(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: TextHandler + ?Sized> TextTransform for H {
    async fn redact(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        for (location, redaction) in redactions {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
