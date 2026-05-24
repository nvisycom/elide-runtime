//! DOCX loader (stub: awaiting full text extraction implementation).
//!
//! Currently produces an empty [`RichTextHandler`] with no pages.

use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, WordFormat};

use super::RichTextHandler;

/// Parameters for [`DocxLoader`].
#[derive(Debug, Default)]
pub struct DocxParams;

/// Loader that creates a [`RichTextHandler`] from DOCX content.
///
/// Text extraction is not yet implemented — produces an empty handler
/// that preserves the raw bytes for round-trip encoding.
#[derive(Debug, Default)]
pub struct DocxLoader;

#[async_trait::async_trait]
impl Loader for DocxLoader {
    type Handler = RichTextHandler;
    type Params = DocxParams;

    #[tracing::instrument(name = "docx.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<RichTextHandler, Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = RichTextHandler::new(DocumentType::Word(WordFormat::Docx), Vec::new(), raw)
            .with_source(source);
        Ok(handler)
    }
}
