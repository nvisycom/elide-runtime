//! Image redaction output type and rendering primitives.

mod instruction;
mod transform;

use futures::StreamExt;
use image::DynamicImage;
use nvisy_core::Error;
pub use instruction::{ImageOutput, ImageRedaction};
pub use transform::ImageTransform;

use crate::document::{SpanEdit, SpanEditStream};
use crate::handler::{ImageData, ImageHandler};

/// Extension trait for handlers that support image redaction.
///
/// Extends [`ImageHandler`] with [`redact_images`](Self::redact_images)
/// which applies a batch of bounding-box image redactions.  The blanket
/// implementation reads the image via [`image_spans`](ImageHandler::image_spans),
/// applies all redactions, and writes back via
/// [`edit_images`](ImageHandler::edit_images).
#[async_trait::async_trait]
pub trait ImageRedact: ImageHandler {
    /// Apply a batch of image redactions, mutating in place.
    async fn redact_images(
        &mut self,
        redactions: &[ImageRedaction<Self::ImageId>],
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: ImageHandler> ImageRedact for H
where
    H::ImageId: Default,
    ImageData: From<ImageData>,
{
    async fn redact_images(
        &mut self,
        redactions: &[ImageRedaction<Self::ImageId>],
    ) -> Result<(), Error> {
        tracing::debug!(
            redaction_count = redactions.len(),
            "applying image redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        // Get the current image from the single span.
        let spans: Vec<_> = self.image_spans().await.collect().await;
        let span = match spans.into_iter().next() {
            Some(s) => s,
            None => return Ok(()),
        };

        let image_data: ImageData = span.data;
        let mut img: DynamicImage = image_data.into_inner();

        for redaction in redactions {
            let region = redaction.bounding_box.to_u32();
            match &redaction.output {
                ImageOutput::Blur { sigma } => {
                    img.apply_gaussian_blur(&region, *sigma);
                }
                ImageOutput::Block { color } => {
                    img.apply_block_overlay(&region, *color);
                }
                ImageOutput::Pixelate { block_size } => {
                    img.apply_pixelate(&region, *block_size);
                }
                ImageOutput::Replace { data } => {
                    let replacement = match image::load_from_memory(data) {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::warn!(
                                region = ?region,
                                error = %e,
                                "failed to decode replacement image data, skipping region"
                            );
                            continue;
                        }
                    };
                    let resized = replacement.resize_exact(
                        region.width,
                        region.height,
                        image::imageops::FilterType::Lanczos3,
                    );
                    image::imageops::overlay(&mut img, &resized, region.x as i64, region.y as i64);
                }
            }
        }

        self.edit_images(SpanEditStream::new(futures::stream::iter(std::iter::once(
            SpanEdit::new(span.id, ImageData::from(img)),
        ))))
        .await?;

        tracing::debug!("image redactions applied");
        Ok(())
    }
}
