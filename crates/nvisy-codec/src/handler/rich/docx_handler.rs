//! DOCX handler (stub: awaiting migration to full Loader/Handler pattern).

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::handler::{Handler, ImageHandler, SpanEditStream, SpanStream, TextHandler};
use crate::handler::text::TextData;
use crate::handler::image::ImageData;

#[derive(Debug)]
pub struct DocxHandler;

impl Handler for DocxHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Docx
    }

    #[tracing::instrument(name = "docx.encode", skip_all)]
    fn encode(&self) -> Result<bytes::Bytes, Error> {
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

    async fn edit_text(
        &mut self,
        _edits: SpanEditStream<'_, (), TextData>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl ImageHandler for DocxHandler {
    type ImageId = ();

    async fn image_spans(&self) -> SpanStream<'_, (), ImageData> {
        SpanStream::new(futures::stream::empty())
    }

    async fn edit_images(
        &mut self,
        _edits: SpanEditStream<'_, (), ImageData>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
