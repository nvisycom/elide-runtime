//! TIFF loader: validates and decodes raw TIFF bytes into a
//! [`TiffHandler`].

use nvisy_core::Error;
use nvisy_core::modality::Image;

use super::TiffHandler;
use crate::Loader;
use crate::content::{ContentData, ContentSource};

/// Loader for TIFF files. Produces one [`TiffHandler`] per input.
#[derive(Debug, Default)]
pub struct TiffLoader;

#[async_trait::async_trait]
impl Loader<Image> for TiffLoader {
    type Handler = TiffHandler;

    #[tracing::instrument(name = "tiff.decode", skip_all, fields(input_bytes, width, height))]
    async fn decode(&self, content: ContentData) -> Result<TiffHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let raw = content.to_bytes();
        let image = image::load_from_memory(&raw)
            .map_err(|e| Error::validation(format!("TIFF decode failed: {e}"), "tiff-loader"))?;
        tracing::Span::current().record("width", image.width());
        tracing::Span::current().record("height", image.height());
        let source = ContentSource::new().with_parent(&parent);
        Ok(TiffHandler::new(image).with_source(source))
    }
}
