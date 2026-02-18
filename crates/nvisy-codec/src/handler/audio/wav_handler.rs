//! WAV handler (stub — awaiting migration to Loader/Handler pattern).

use bytes::Bytes;

use nvisy_core::error::Error;
use nvisy_core::fs::DocumentType;

use crate::document::{SpanEditStream, SpanStream};
use crate::handler::Handler;

#[derive(Debug, Clone)]
pub struct WavHandler {
    pub(crate) bytes: Bytes,
}

impl WavHandler {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

#[async_trait::async_trait]
impl Handler for WavHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Wav
    }

    fn encode(&self) -> Result<Vec<u8>, Error> {
        Ok(self.bytes.to_vec())
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
