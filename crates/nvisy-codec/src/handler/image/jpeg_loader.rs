//! JPEG loader: validates and decodes raw JPEG bytes into a
//! [`JpegHandler`].

use async_trait::async_trait;
use nvisy_core::Error;
use nvisy_core::modality::Image;

use super::JpegHandler;
use crate::content::{ContentData, ContentSource};
use crate::core::Loader;

/// Loader for JPEG files. Produces one [`JpegHandler`] per input.
#[derive(Debug, Default)]
pub struct JpegLoader;

#[async_trait]
impl Loader<Image> for JpegLoader {
    type Handler = JpegHandler;

    #[tracing::instrument(name = "jpeg.decode", skip_all, fields(input_bytes, width, height))]
    async fn decode(&self, content: ContentData) -> Result<JpegHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let raw = content.to_bytes();
        let image = image::load_from_memory(&raw)
            .map_err(|e| Error::validation(format!("JPEG decode failed: {e}"), "jpeg-loader"))?;
        tracing::Span::current().record("width", image.width());
        tracing::Span::current().record("height", image.height());
        let source = ContentSource::new().with_parent(&parent);
        Ok(JpegHandler::new(image).with_source(source))
    }
}
