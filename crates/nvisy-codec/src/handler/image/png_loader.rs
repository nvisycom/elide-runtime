//! PNG loader: validates and decodes raw PNG bytes into a
//! [`PngHandler`].

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::handler::{Loader, PngHandler};

/// Parameters for [`PngLoader`].
#[derive(Debug, Default)]
pub struct PngParams;

/// Loader that validates and decodes PNG files.
///
/// Produces a single [`PngHandler`] per input.
#[derive(Debug)]
pub struct PngLoader;

#[async_trait::async_trait]
impl Loader for PngLoader {
    type Handler = PngHandler;
    type Params = PngParams;

    #[tracing::instrument(name = "png.decode", skip_all, fields(input_bytes, width, height))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<PngHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let image = super::decode_image(content, "png-loader")?;
        tracing::Span::current().record("width", image.width());
        tracing::Span::current().record("height", image.height());
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = PngHandler::new(image).with_source(source);
        Ok(handler)
    }
}
