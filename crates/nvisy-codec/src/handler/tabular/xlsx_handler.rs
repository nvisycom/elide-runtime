//! XLSX handler (stub: awaiting full spreadsheet support).
//!
//! Implements [`TabularHandler`] only — the underlying ZIP-of-XML
//! structure does not map to flat byte offsets, so the handler does
//! not implement [`TextHandler`].
//!
//! [`TabularHandler`]: crate::handler::TabularHandler
//! [`TextHandler`]: crate::handler::TextHandler

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_ontology::entity::TabularLocation;

use crate::document::LocationStream;
use crate::handler::{Handler, TabularHandler, TextData};
use crate::transform::{Redactions, TabularRedaction};

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
impl TabularHandler for XlsxHandler {
    fn locations(&self) -> LocationStream<'_, TabularLocation> {
        LocationStream::empty()
    }

    async fn read(&self, _location: &TabularLocation) -> Option<TextData> {
        None
    }

    async fn redact(
        &mut self,
        _redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
