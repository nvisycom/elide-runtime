//! DOCX handler (stub: awaiting migration to full Loader/Handler pattern).

use nvisy_core::Error;
use nvisy_core::fs::{DocumentType, WordFormat};
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use crate::document::SpanStream;
use crate::handler::image::ImageData;
use crate::handler::rich::RichImageSpan;
use crate::handler::text::TextData;
use crate::handler::{Handler, ImageHandler, TextHandler};

#[derive(Debug)]
pub struct DocxHandler {
    source: ContentSource,
}

impl DocxHandler {
    /// Create a new stub handler.
    pub fn new() -> Self {
        Self {
            source: ContentSource::new(),
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }
}

impl Handler for DocxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Word(WordFormat::Docx)
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "docx.encode", skip_all)]
    fn encode(&self) -> Result<ContentData, Error> {
        Err(Error::validation(
            "encode not supported for DOCX",
            "docx-handler",
        ))
    }
}

#[async_trait::async_trait]
impl TextHandler for DocxHandler {
    type TextId = ();

    async fn text_spans(&self) -> SpanStream<'_, (), TextData> {
        SpanStream::new(futures::stream::empty())
    }

    async fn edit_text(&mut self, _edits: SpanStream<'_, (), TextData>) -> Result<(), Error> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl ImageHandler for DocxHandler {
    type ImageId = RichImageSpan;

    async fn image_spans(&self) -> SpanStream<'_, RichImageSpan, ImageData> {
        SpanStream::new(futures::stream::empty())
    }

    async fn edit_images(
        &mut self,
        _edits: SpanStream<'_, RichImageSpan, ImageData>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
