//! XLSX loader (stub — awaiting full spreadsheet support).

use nvisy_core::error::Error;
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

    async fn load(
        &self,
        _content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<XlsxHandler>>, Error> {
        let handler = XlsxHandler;
        let doc = Document::new(handler).with_parent(_content);
        Ok(vec![doc])
    }
}
