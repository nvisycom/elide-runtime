//! DOCX handler (stub — awaiting migration to Loader/Handler pattern).

use nvisy_core::error::Error;
use nvisy_core::fs::DocumentType;

use crate::document::edit_stream::SpanEditStream;
use crate::document::view_stream::SpanStream;
use crate::handler::Handler;

#[derive(Debug)]
pub struct DocxHandler;

#[async_trait::async_trait]
impl Handler for DocxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Docx
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
