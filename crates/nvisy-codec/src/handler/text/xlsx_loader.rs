//! XLSX loader (stub: awaiting full spreadsheet support).

use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_core::content::ContentSource;

use crate::handler::{Loader, XlsxHandler};

/// Parameters for [`XlsxLoader`].
#[derive(Debug, Default)]
pub struct XlsxParams;

/// Loader that parses XLSX spreadsheets.
///
/// Produces a single [`XlsxHandler`] per input.
#[derive(Debug, Default)]
pub struct XlsxLoader;

#[async_trait::async_trait]
impl Loader for XlsxLoader {
    type Handler = XlsxHandler;
    type Params = XlsxParams;

    #[tracing::instrument(name = "xlsx.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<XlsxHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = XlsxHandler::new().with_source(source);
        Ok(handler)
    }
}
