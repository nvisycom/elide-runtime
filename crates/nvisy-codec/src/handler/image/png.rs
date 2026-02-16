//! PNG handler (stub — awaiting migration to Loader/Handler pattern).

use bytes::Bytes;
use image::DynamicImage;

use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::fs::DocumentType;

use crate::document::edit_stream::SpanEditStream;
use crate::document::view_stream::SpanStream;
use crate::handler::Handler;
use crate::render::image::AsImage;

#[derive(Debug, Clone)]
pub struct PngHandler {
    pub(crate) bytes: Bytes,
}

impl PngHandler {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

#[async_trait::async_trait]
impl Handler for PngHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Png
    }

    type SpanId = ();
    type SpanData = ();

    async fn view_spans(&self) -> SpanStream<'_, (), ()> {
        SpanStream::new(futures::stream::empty())
    }

    async fn edit_spans(
        &mut self,
        _edits: SpanEditStream<'_, (), ()>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl AsImage for PngHandler {
    fn decode(&self) -> Result<DynamicImage, Error> {
        image::load_from_memory(&self.bytes).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PNG decode failed: {e}"))
        })
    }

    fn encode(image: &DynamicImage) -> Result<Self, Error> {
        let mut buf = std::io::Cursor::new(Vec::new());
        image.write_to(&mut buf, image::ImageFormat::Png).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("PNG encode failed: {e}"))
        })?;
        Ok(Self::new(Bytes::from(buf.into_inner())))
    }
}
