//! TIFF loader: validates and decodes raw TIFF bytes into a
//! [`TiffHandler`].

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};

use crate::handler::{ImageData, Loader, TiffHandler};

/// Parameters for [`TiffLoader`].
#[derive(Debug, Default)]
pub struct TiffParams;

/// Loader that validates and decodes TIFF files.
///
/// Produces a single [`TiffHandler`] per input.
#[derive(Debug, Default)]
pub struct TiffLoader;

#[async_trait::async_trait]
impl Loader for TiffLoader {
    type Handler = TiffHandler;
    type Params = TiffParams;

    #[tracing::instrument(name = "tiff.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<TiffHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let image = ImageData::decode(content, "tiff-loader")?.into_inner();
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = TiffHandler::new(image).with_source(source);
        Ok(handler)
    }
}
