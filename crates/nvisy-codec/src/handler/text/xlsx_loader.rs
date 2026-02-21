//! XLSX loader (stub — awaiting full spreadsheet support).

use nvisy_core::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, XlsxHandler};

/// Parameters for [`XlsxLoader`].
#[derive(Debug, Default)]
pub struct XlsxParams;

/// Loader that parses XLSX spreadsheets.
///
/// Produces a single [`Document<XlsxHandler>`] per input.
#[derive(Debug)]
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
    ) -> Result<Vec<Document<XlsxHandler>>, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let handler = XlsxHandler;
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
