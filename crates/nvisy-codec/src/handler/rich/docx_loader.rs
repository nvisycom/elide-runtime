//! DOCX loader (stub: awaiting real implementation).

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::handler::{DocxHandler, Loader};

/// Parameters for [`DocxLoader`].
#[derive(Debug, Default)]
pub struct DocxParams;

/// Loader that creates a stub DOCX handler.
///
/// Produces a single [`DocxHandler`] per input.
#[derive(Debug)]
pub struct DocxLoader;

#[async_trait::async_trait]
impl Loader for DocxLoader {
    type Handler = DocxHandler;
    type Params = DocxParams;

    #[tracing::instrument(name = "docx.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<DocxHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = DocxHandler::new().with_source(source);
        Ok(handler)
    }
}
