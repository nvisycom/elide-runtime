//! DOCX loader (stub: awaiting text extraction implementation).
//!
//! Wraps the raw bytes into a [`DocxHandler`] that preserves the
//! input for round-trip encoding but exposes no text chunks.

use async_trait::async_trait;
use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::modality::Text;

use super::DocxHandler;

/// Loader that wraps raw DOCX bytes.
#[derive(Debug, Default)]
pub struct DocxLoader;

#[async_trait]
impl Loader<Text> for DocxLoader {
    type Handler = DocxHandler;

    #[tracing::instrument(name = "docx.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<DocxHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let source = ContentSource::new().with_parent(&parent);
        Ok(DocxHandler::new(raw).with_source(source))
    }
}
