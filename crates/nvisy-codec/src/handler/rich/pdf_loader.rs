//! PDF loader (stub: awaiting real implementation).

use nvisy_core::Error;
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

    #[tracing::instrument(name = "pdf.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Document<PdfHandler>, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let handler = PdfHandler;
        let doc = Document::new(handler).with_parent(content);
        Ok(doc)
    }
}
