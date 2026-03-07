//! XLSX handler (stub: awaiting full spreadsheet support).

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

#[derive(Debug)]
pub struct XlsxHandler;

impl Handler for XlsxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Xlsx
    }

    #[tracing::instrument(name = "xlsx.encode", skip_all)]
    fn encode(&self) -> Result<ContentData, Error> {
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

    async fn edit_text(&mut self, _edits: SpanEditStream<'_, (), TextData>) -> Result<(), Error> {
        Ok(())
    }
}
