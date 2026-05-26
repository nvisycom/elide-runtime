//! XLSX handler (stub: awaiting full spreadsheet support).
//!
//! Implements [`Handle`] only — the underlying ZIP-of-XML
//! structure does not map to flat byte offsets, so the handler does
//! not implement [`TextHandler`].
//!
//! [`Handle`]: nvisy_codec::handler::Handle
//! [`TextHandler`]: nvisy_codec::handler::TextHandler

use nvisy_codec::document::LocationStream;
use nvisy_codec::handler::{Handle, Handler, TabularRedaction, TextData};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_ontology::modality::Tabular;

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
impl Handle<Tabular> for XlsxHandler {
    fn locations(&self) -> LocationStream<'_, Tabular> {
        LocationStream::empty()
    }

    async fn read(&self, _location: &Tabular) -> Option<TextData> {
        None
    }

    async fn redact_at(
        &mut self,
        _location: &Tabular,
        _redaction: TabularRedaction,
    ) -> Result<(), Error> {
        Ok(())
    }
}
