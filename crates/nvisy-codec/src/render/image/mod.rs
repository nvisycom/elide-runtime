//! Image redaction output type and rendering primitives.

mod output;

pub use output::ImageRedactionOutput;

mod blur;
mod block;
mod pixelate;

use blur::apply_gaussian_blur;
use block::apply_block_overlay;
use pixelate::apply_pixelate;

use image::DynamicImage;
use futures::StreamExt;

use crate::document::edit_stream::SpanEditStream;
use crate::handler::{Handler, SpanEdit};
use nvisy_core::error::Error;
use nvisy_core::math::{BoundingBox, BoundingBoxU32};

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

        for r in redactions {
            let region = BoundingBoxU32::from(&r.bounding_box);
            let regions = std::slice::from_ref(&region);
            match &r.output {
                ImageRedactionOutput::Blur { sigma } => {
                    img = apply_gaussian_blur(&img, regions, *sigma);
                }
                ImageRedactionOutput::Block { color } => {
                    img = apply_block_overlay(&img, regions, *color);
                }
                ImageRedactionOutput::Pixelate { block_size } => {
                    img = apply_pixelate(&img, regions, *block_size);
                }
                ImageRedactionOutput::Synthesize => {
                    img = apply_block_overlay(&img, regions, [0, 0, 0, 255]);
                }
            }
        }

        self.edit_spans(SpanEditStream::new(futures::stream::iter(
            std::iter::once(SpanEdit {
                id: span.id,
                data: Self::SpanData::from(img),
            }),
        )))
        .await?;

        Ok(())
    }
}
