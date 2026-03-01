//! XLSX handler (stub: awaiting full spreadsheet support).

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::handler::{Handler, SpanEditStream, SpanStream, TextHandler};
use crate::handler::text::TextData;

#[derive(Debug)]
pub struct XlsxHandler;

impl Handler for XlsxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Xlsx
    }

    #[tracing::instrument(name = "xlsx.encode", skip_all)]
    fn encode(&self) -> Result<bytes::Bytes, Error> {
        Err(Error::validation(
            "encode not supported for XLSX",
            "xlsx-handler",
        ))
    }
}

#[async_trait::async_trait]
impl TextHandler for XlsxHandler {
    type TextId = ();

    async fn text_spans(&self) -> SpanStream<'_, (), TextData> {
        SpanStream::new(futures::stream::empty())
    }

    async fn edit_text(
        &mut self,
        _edits: SpanEditStream<'_, (), TextData>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
