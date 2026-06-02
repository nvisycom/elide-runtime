//! XLSX handler (stub: awaiting full spreadsheet support).
//!
//! Implements [`Handle`] only — the underlying ZIP-of-XML
//! structure does not map to flat byte offsets, so the handler does
//! not implement [`TextHandler`].
//!
//! [`Handle`]: nvisy_codec::core::Handle
//! [`TextHandler`]: nvisy_codec::handler::TextHandler

use nvisy_codec::core::{Handle, LocationStream};
use nvisy_codec::handler::{Handler, TabularHandle, TabularRedaction, TextData};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, DocumentType, SpreadsheetFormat};
use nvisy_core::modality::Tabular;

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

impl TabularHandle for XlsxHandler {
    fn has_header(&self) -> bool {
        // XLSX always carries a typed schema (cell types, named
        // ranges, header row). Stays `true` even though the stub
        // handler returns no locations today.
        true
    }
}
