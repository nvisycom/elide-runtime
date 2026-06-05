//! PNG loader: validates and decodes raw PNG bytes into a
//! [`PngHandler`].

use async_trait::async_trait;
use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::modality::Image;

use super::PngHandler;

/// Loader for PNG files. Produces one [`PngHandler`] per input.
#[derive(Debug, Default)]
pub struct PngLoader;

#[async_trait]
impl Loader<Image> for PngLoader {
    type Handler = PngHandler;

    #[tracing::instrument(name = "png.decode", skip_all, fields(input_bytes, width, height))]
    async fn decode(&self, content: ContentData) -> Result<PngHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let raw = content.to_bytes();
        let image = image::load_from_memory(&raw)
            .map_err(|e| Error::validation(format!("PNG decode failed: {e}"), "png-loader"))?;
        tracing::Span::current().record("width", image.width());
        tracing::Span::current().record("height", image.height());
        let source = ContentSource::new().with_parent(&parent);
        Ok(PngHandler::new(image).with_source(source))
    }
}
