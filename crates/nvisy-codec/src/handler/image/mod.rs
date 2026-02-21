//! Image format handlers and loaders.

mod jpeg_handler;
mod jpeg_loader;

mod png_handler;
mod png_loader;

pub use png_handler::PngHandler;
pub use png_loader::{PngLoader, PngParams};

pub use jpeg_handler::JpegHandler;
pub use jpeg_loader::{JpegLoader, JpegParams};

use image::DynamicImage;
use nvisy_core::Error;
use nvisy_core::io::ContentData;

/// Decode raw bytes into a [`DynamicImage`].
///
/// Shared by all image loaders.
pub(crate) fn decode_image(content: &ContentData, origin: &str) -> Result<DynamicImage, Error> {
    let raw = content.to_bytes();
    image::load_from_memory(&raw)
        .map_err(|e| Error::validation(format!("image decode failed: {e}"), origin))
}

/// Implement [`Handler`] + [`ImageHandler`] + inherent methods for an
/// image handler struct that holds a single `DynamicImage`.
macro_rules! impl_image_handler {
    ($handler:ident, $doc_type:expr, $fmt:expr, $origin:literal, $encode_name:literal) => {
        #[async_trait::async_trait]
        impl crate::handler::Handler for $handler {
            fn document_type(&self) -> nvisy_core::fs::DocumentType {
                $doc_type
            }

            #[tracing::instrument(name = $encode_name, skip_all, fields(output_bytes))]
            fn encode(&self) -> Result<Vec<u8>, nvisy_core::Error> {
                let mut buf = std::io::Cursor::new(Vec::new());
                self.image
                    .write_to(&mut buf, $fmt)
                    .map_err(|e| {
                        nvisy_core::Error::validation(
                            format!("encode failed: {e}"),
                            $origin,
                        )
                    })?;
                let out = buf.into_inner();
                tracing::Span::current().record("output_bytes", out.len());
                Ok(out)
            }

            type SpanId = ();
            type SpanData = image::DynamicImage;

            async fn view_spans(
                &self,
            ) -> crate::document::SpanStream<'_, (), image::DynamicImage> {
                crate::document::SpanStream::new(futures::stream::iter(std::iter::once(
                    crate::handler::Span {
                        id: (),
                        data: self.image.clone(),
                    },
                )))
            }

            async fn edit_spans(
                &mut self,
                edits: crate::document::SpanEditStream<'_, (), image::DynamicImage>,
            ) -> Result<(), nvisy_core::Error> {
                use futures::StreamExt;
                let edits: Vec<_> = edits.collect().await;
                if let Some(edit) = edits.into_iter().next() {
                    self.image = edit.data;
                }
                Ok(())
            }
        }

        impl crate::transform::ImageHandler for $handler {}

        impl $handler {
            /// Create a handler from an already-decoded image.
            pub fn new(image: image::DynamicImage) -> Self {
                Self { image }
            }

            /// Reference to the decoded image.
            pub fn image(&self) -> &image::DynamicImage {
                &self.image
            }
        }
    };
}

pub(crate) use impl_image_handler;
