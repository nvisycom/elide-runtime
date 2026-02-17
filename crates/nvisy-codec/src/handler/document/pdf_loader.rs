//! PDF loader (stub — awaiting real implementation).

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{Loader, PdfHandler};

/// Parameters for [`PdfLoader`].
#[derive(Debug, Default)]
pub struct PdfParams;

/// Loader that creates a stub PDF handler.
///
/// Produces a single [`Document<PdfHandler>`] per input.
#[derive(Debug)]
pub struct PdfLoader;

#[async_trait::async_trait]
impl Loader for PdfLoader {
    type Handler = PdfHandler;
    type Params = PdfParams;

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<PdfHandler>>, Error> {
        let handler = PdfHandler;
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}
