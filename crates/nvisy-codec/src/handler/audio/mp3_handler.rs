//! MP3 handler (stub — awaiting migration to Loader/Handler pattern).

use bytes::Bytes;

use nvisy_core::error::Error;
use nvisy_core::fs::DocumentType;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::Handler;

#[derive(Debug, Clone)]
pub struct Mp3Handler {
    pub(crate) bytes: Bytes,
}

impl Mp3Handler {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

#[async_trait::async_trait]
impl Handler for Mp3Handler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Mp3
    }

    #[tracing::instrument(name = "mp3.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = self.bytes.to_vec();
        tracing::Span::current().record("output_bytes", bytes.len());
        Ok(bytes)
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
