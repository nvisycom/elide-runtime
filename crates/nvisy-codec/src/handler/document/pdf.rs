//! PDF handler (stub — awaiting migration to Loader/Handler pattern).

use nvisy_core::error::Error;
use nvisy_ontology::entity::DocumentType;

use crate::document::edit_stream::SpanEditStream;
use crate::document::view_stream::SpanStream;
use crate::handler::Handler;

#[derive(Debug)]
pub struct PdfHandler;

#[async_trait::async_trait]
impl Handler for PdfHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Pdf
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
