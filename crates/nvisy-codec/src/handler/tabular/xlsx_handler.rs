//! XLSX handler (stub: awaiting full spreadsheet support).
//!
//! Decodes to an empty handler — emits no chunks, reads return `None`,
//! redactions are no-ops, encode fails. Wired into the registry so
//! downstream code can resolve XLSX without a runtime panic, while
//! signalling that meaningful tabular access isn't implemented yet.

use nvisy_core::Error;
use nvisy_core::modality::{Tabular, TabularLocation, TextData};
use nvisy_core::redaction::Redactions;

use super::XlsxLoader;
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handler};
use crate::handler::tabular::TabularHandle;
use crate::{Format, FormatId};

/// Stable [`FormatId`] for the XLSX codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.tabular.xlsx");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format::new::<Tabular, _>(FORMAT_ID.clone(), XlsxLoader)
        .with_extensions(["xlsx"])
        .with_content_types(["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"])
}

#[derive(Debug, Default)]
pub struct XlsxHandler {
    source: ContentSource,
}

impl XlsxHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }
}

#[async_trait::async_trait]
impl Handler<Tabular> for XlsxHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
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

    async fn next_chunk(&mut self) -> Result<Option<Chunk<Tabular>>, Error> {
        Ok(None)
    }

    async fn read(&self, _location: &TabularLocation) -> Result<Option<TextData>, Error> {
        Ok(None)
    }

    async fn redact(&mut self, _redactions: Redactions<Tabular>) -> Result<(), Error> {
        Ok(())
    }
}

impl TabularHandle for XlsxHandler {
    fn has_header(&self) -> bool {
        // XLSX always carries a typed schema.
        true
    }
}
