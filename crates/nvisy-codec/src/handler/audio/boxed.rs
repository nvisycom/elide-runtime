//! [`BoxedAudioHandler`]: type-erased wrapper over all audio handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::modality::Audio;

use super::AudioData;
use crate::document::LocationStream;
use crate::handler::{AudioHandler, AudioRedaction, Handler};

/// A type-erased audio handler backed by a boxed trait object.
pub struct BoxedAudioHandler(Box<dyn AudioHandler>);

impl BoxedAudioHandler {
    /// Wrap any concrete audio handler into a type-erased box.
    pub fn new<H: AudioHandler>(handler: H) -> Self {
        Self(Box::new(handler))
    }
}

impl fmt::Debug for BoxedAudioHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedAudioHandler")
            .field(&self.0.document_type())
            .finish()
    }
}

impl Handler for BoxedAudioHandler {
    fn document_type(&self) -> DocumentType {
        Handler::document_type(self.0.as_ref())
    }

    fn source(&self) -> ContentSource {
        Handler::source(self.0.as_ref())
    }

    fn encode(&self) -> Result<ContentData, Error> {
        Handler::encode(self.0.as_ref())
    }
}

#[async_trait::async_trait]
impl AudioHandler for BoxedAudioHandler {
    fn locations(&self) -> LocationStream<'_, Audio> {
        self.0.locations()
    }

    async fn read(&self, location: &Audio) -> Option<AudioData> {
        self.0.read(location).await
    }

    async fn redact_at(
        &mut self,
        location: &Audio,
        redaction: AudioRedaction,
    ) -> Result<(), Error> {
        self.0.redact_at(location, redaction).await
    }
}
