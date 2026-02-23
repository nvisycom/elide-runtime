//! Image redaction output type and rendering primitives.

mod output;
mod transform;

pub use output::ImageRedactionOutput;
pub use transform::ImageTransform;

use image::DynamicImage;
use futures::StreamExt;

use crate::document::SpanEditStream;
use crate::handler::{Handler, SpanEdit};
use nvisy_core::Error;
use nvisy_core::math::BoundingBox;

/// A located image redaction: pairs a bounding box with an
/// [`ImageRedactionOutput`] that carries the method-specific parameters.
pub struct ImageRedaction {
    /// Bounding box of the region to redact.
    pub bounding_box: BoundingBox,
    /// The redaction output that determines the rendering method.
    pub output: ImageRedactionOutput,
}

/// Trait for handlers that support image redaction.
///
/// Extends [`Handler`] with [`redact_spans`](Self::redact_spans) which
/// applies a batch of bounding-box image redactions.  The provided
/// default implementation reads the image via [`view_spans`](Handler::view_spans),
/// applies all redactions, and writes back via [`edit_spans`](Handler::edit_spans).
#[async_trait::async_trait]
pub trait ImageHandler: Handler
where
    Self::SpanData: Into<DynamicImage> + From<DynamicImage>,
{
    /// Apply a batch of image redactions, mutating in place.
    async fn redact_spans(
        &mut self,
        redactions: &[ImageRedaction],
    ) -> Result<(), Error> {
        tracing::debug!(redaction_count = redactions.len(), "applying image redactions");
        if redactions.is_empty() {
            return Ok(());
        }

        // Get the current image from the single span.
        let spans: Vec<_> = self.view_spans().await.collect().await;
        let span = match spans.into_iter().next() {
            Some(s) => s,
            None => return Ok(()),
        };

        let mut img: DynamicImage = span.data.into();

        for redaction in redactions {
            let region = redaction.bounding_box.to_u32();
            match &redaction.output {
                ImageRedactionOutput::Blur { sigma } => {
                    img.apply_gaussian_blur(&region, *sigma);
                }
                ImageRedactionOutput::Block { color } => {
                    img.apply_block_overlay(&region, *color);
                }
                ImageRedactionOutput::Pixelate { block_size } => {
                    img.apply_pixelate(&region, *block_size);
                }
                ImageRedactionOutput::Replace { data } => {
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
                    image::imageops::overlay(
                        &mut img,
                        &resized,
                        region.x as i64,
                        region.y as i64,
                    );
                }
            }
        }

        self.edit_spans(SpanEditStream::new(futures::stream::iter(
            std::iter::once(SpanEdit::new(span.id, Self::SpanData::from(img))),
        )))
        .await?;

        tracing::debug!("image redactions applied");
        Ok(())
    }
}
