//! XLSX handler (stub: awaiting full spreadsheet support).

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_ontology::entity::TextLocation;

use crate::document::LocationStream;
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};
use crate::transform::{Redactions, TextRedaction};

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
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        LocationStream::empty()
    }

    async fn read(&self, _location: &TextLocation) -> Option<TextData> {
        None
    }

    async fn redact(
        &mut self,
        _redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
