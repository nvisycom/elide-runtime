//! [`ImageTransform`] async trait and blanket implementation.

use futures::StreamExt;
use image::DynamicImage;
use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::instruction::{ImageOutput, ImageRedaction};
use super::ops::ImageOps;
use crate::document::{Span, SpanStream};
use crate::handler::{ImageData, ImageHandler};

const TARGET: &str = "nvisy_codec::transform::image";

/// Extension trait for handlers that support image redaction.
#[async_trait::async_trait]
pub trait ImageTransform: ImageHandler {
    /// Apply a batch of image redactions, mutating in place.
    async fn redact_images(
        &mut self,
        redactions: &[ImageRedaction<ImageLocation>],
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: ImageHandler> ImageTransform for H {
    async fn redact_images(
        &mut self,
        redactions: &[ImageRedaction<ImageLocation>],
    ) -> Result<(), Error> {
        tracing::debug!(
            target: TARGET,
            redaction_count = redactions.len(),
            "applying image redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        let spans: Vec<_> = self.image_spans().await.collect().await;
        let span = match spans.into_iter().next() {
            Some(s) => s,
            None => return Ok(()),
        };

        let image_data: ImageData = span.data;
        let mut img: DynamicImage = image_data.into_inner();

        for redaction in redactions {
            let region = redaction.bounding_box.to_pixel();
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
                                target: TARGET,
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

        self.edit_images(SpanStream::new(futures::stream::iter(std::iter::once(
            Span::new(span.id, ImageData::from(img)),
        ))))
        .await?;

        tracing::debug!(target: TARGET, "image redactions applied");
        Ok(())
    }
}
