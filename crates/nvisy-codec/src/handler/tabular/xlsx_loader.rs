//! XLSX loader (stub: awaiting full spreadsheet support).

use async_trait::async_trait;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::modality::Tabular;

use super::XlsxHandler;
use crate::core::Loader;

/// Loader for XLSX spreadsheets. Produces a single (stub) [`XlsxHandler`].
#[derive(Debug, Default)]
pub struct XlsxLoader;

#[async_trait]
impl Loader<Tabular> for XlsxLoader {
    type Handler = XlsxHandler;

    #[tracing::instrument(name = "xlsx.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<XlsxHandler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let source = ContentSource::new().with_parent(&parent);
        Ok(XlsxHandler::new().with_source(source))
    }
}
