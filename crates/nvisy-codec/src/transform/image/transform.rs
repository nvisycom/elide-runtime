//! [`ImageTransform`]: blanket-impl extension that iterates a
//! [`Redactions`] collection per-location, dispatching to the
//! handler's [`ImageHandler::redact_at`] hook.

use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::instruction::ImageRedaction;
use crate::handler::ImageHandler;
use crate::transform::Redactions;

/// Apply a batch of image redactions, one per location.
///
/// Blanket-implemented for every [`ImageHandler`]: the handler only
/// implements the narrow [`ImageHandler::redact_at`] hook; iteration
/// over the [`Redactions`] collection lives here so handlers can't
/// accidentally drop or reorder the location key.
#[async_trait::async_trait]
pub trait ImageTransform: ImageHandler {
    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    async fn redact(
        &mut self,
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: ImageHandler + ?Sized> ImageTransform for H {
    async fn redact(
        &mut self,
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error> {
        for (location, redaction) in redactions {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
