//! DOCX loader (stub — awaiting real implementation).

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, DocxHandler};

/// Parameters for [`DocxLoader`].
#[derive(Debug, Default)]
pub struct DocxParams;

/// Loader that creates a stub DOCX handler.
///
/// Produces a single [`Document<DocxHandler>`] per input.
#[derive(Debug)]
pub struct DocxLoader;

#[async_trait::async_trait]
impl Loader for DocxLoader {
    type Handler = DocxHandler;
    type Params = DocxParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<DocxHandler>>, Error> {
        let handler = DocxHandler;
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
