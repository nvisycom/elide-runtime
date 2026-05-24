//! Image-handler trait + supporting infrastructure.
//!
//! The trait, redaction shape, `ImageData`, ops helpers, and the
//! `apply_image_redaction` + `impl_image_handler!` shared utilities
//! live here; concrete per-format implementations (PNG, JPEG, TIFF)
//! live in `nvisy-formats`.

use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::Redactions;

mod apply;
mod boxed;
mod image_data;
mod image_handler_macro;
mod instruction;
mod ops;

pub use self::apply::apply_image_redaction;
pub use self::boxed::BoxedImageHandler;
pub use self::image_data::ImageData;
pub use self::instruction::{ImageOutput, ImageRedaction};
// `impl_image_handler!` is `#[macro_export]`-ed in
// `image_handler_macro.rs`, so it lives at `::nvisy_codec::impl_image_handler`.

/// Capability trait for handlers that expose image content.
///
/// Handlers implement three narrow operations:
/// - [`locations`]: cheap, identity-only stream of [`ImageLocation`]s.
/// - [`read`]: fetch the payload at a given location (cropped to
///   the location's bounding box).
/// - [`redact_at`]: apply a single redaction to a single location.
///
/// Batched redaction is provided by [`redact`], which loops
/// [`redact_at`] in insertion order.
///
/// [`locations`]: ImageHandler::locations
/// [`read`]: ImageHandler::read
/// [`redact_at`]: ImageHandler::redact_at
/// [`redact`]: ImageHandler::redact
#[async_trait::async_trait]
pub trait ImageHandler: Handler {
    /// Async stream of [`ImageLocation`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, ImageLocation>;

    /// Read the image data at the given location (crop the bounding box).
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &ImageLocation) -> Option<ImageData>;

    /// Apply a single redaction at the bounding box identified by
    /// `location`, mutating in place.
    async fn redact_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error>;

    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    ///
    /// The default loops [`redact_at`] in [`Redactions`] insertion
    /// order; handlers with ordering constraints override this
    /// default.
    ///
    /// [`redact_at`]: ImageHandler::redact_at
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
