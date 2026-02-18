//! XLSX handler (stub — awaiting full spreadsheet support).

use nvisy_core::error::Error;
use nvisy_core::fs::DocumentType;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::Handler;

#[derive(Debug)]
pub struct XlsxHandler;

#[async_trait::async_trait]
impl Handler for XlsxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Xlsx
    }

    type SpanId = ();
    type SpanData = ();

    async fn view_spans(&self) -> SpanStream<'_, (), ()> {
        SpanStream::new(futures::stream::empty())
    }

    async fn edit_spans(
        &mut self,
        _edits: SpanEditStream<'_, (), ()>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
