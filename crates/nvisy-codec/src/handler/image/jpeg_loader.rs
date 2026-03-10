//! JPEG loader: validates and decodes raw JPEG bytes into a
//! [`JpegHandler`].

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::handler::{ImageData, JpegHandler, Loader};

/// Parameters for [`JpegLoader`].
#[derive(Debug, Default)]
pub struct JpegParams;

/// Loader that validates and decodes JPEG files.
///
/// Produces a single [`JpegHandler`] per input.
#[derive(Debug, Default)]
pub struct JpegLoader;

#[async_trait::async_trait]
impl Loader for JpegLoader {
    type Handler = JpegHandler;
    type Params = JpegParams;

    #[tracing::instrument(name = "jpeg.decode", skip_all, fields(input_bytes, width, height))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<JpegHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let image = ImageData::decode(content, "jpeg-loader")?.into_inner();
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = JpegHandler::new(image).with_source(source);
        Ok(handler)
    }
}
