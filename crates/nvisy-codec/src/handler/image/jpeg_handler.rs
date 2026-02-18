//! JPEG handler — holds a decoded image and provides single-span access
//! via [`Handler`].
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields exactly one [`Span`] whose data is the
//! current [`DynamicImage`].  [`Handler::edit_spans`] replaces the image
//! in-place.

use image::DynamicImage;

use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::fs::DocumentType;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::{Handler, Span};
use crate::transform::ImageHandler;

use futures::StreamExt;

/// Handler for loaded JPEG content.
///
/// Stores the decoded [`DynamicImage`] directly.  The raw JPEG bytes
/// can be produced on demand via [`JpegHandler::encode_bytes`].
#[derive(Debug, Clone)]
pub struct JpegHandler {
    image: DynamicImage,
}

impl JpegHandler {
    /// Create a handler from an already-decoded image.
    pub fn new(image: DynamicImage) -> Self {
        Self { image }
    }

    /// Reference to the decoded image.
    pub fn image(&self) -> &DynamicImage {
        &self.image
    }

    /// Encode the current image to JPEG bytes.
    pub fn encode_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buf = std::io::Cursor::new(Vec::new());
        self.image
            .write_to(&mut buf, image::ImageFormat::Jpeg)
            .map_err(|e| Error::new(ErrorKind::Runtime, format!("JPEG encode failed: {e}")))?;
        Ok(buf.into_inner())
    }
}

#[async_trait::async_trait]
impl Handler for JpegHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Jpeg
    }

    type SpanId = ();
    type SpanData = DynamicImage;

    async fn view_spans(&self) -> SpanStream<'_, (), DynamicImage> {
        SpanStream::new(futures::stream::iter(std::iter::once(Span {
            id: (),
            data: self.image.clone(),
        })))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, (), DynamicImage>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if let Some(edit) = edits.into_iter().next() {
            self.image = edit.data;
        }
        Ok(())
    }
}

impl ImageHandler for JpegHandler {}
