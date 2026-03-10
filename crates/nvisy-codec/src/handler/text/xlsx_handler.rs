//! XLSX handler (stub: awaiting full spreadsheet support).

use nvisy_core::Error;
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_core::content::ContentData;
use nvisy_core::content::ContentSource;

use crate::document::SpanStream;
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

#[derive(Debug, Default)]
pub struct XlsxHandler {
    source: ContentSource,
}

impl XlsxHandler {
    /// Create a new stub handler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }
}

impl Handler for XlsxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Spreadsheet(SpreadsheetFormat::Xlsx)
    }

    fn source(&self) -> ContentSource {
        self.source
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

    async fn edit_text(&mut self, _edits: SpanStream<'_, (), TextData>) -> Result<(), Error> {
        Ok(())
    }
}
