//! [`BoxedTextHandler`]: type-erased wrapper over all text handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::TextLocation;

use super::TextData;
use crate::document::LocationStream;
use crate::handler::{Handler, TextHandler, TextRedaction};

/// A type-erased text handler backed by a boxed trait object.
pub struct BoxedTextHandler(Box<dyn TextHandler>);

impl BoxedTextHandler {
    /// Wrap any concrete text handler.
    pub fn new(handler: impl TextHandler + 'static) -> Self {
        Self(Box::new(handler))
    }
}

impl fmt::Debug for BoxedTextHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedTextHandler")
            .field(&self.0.document_type())
            .finish()
    }
}

impl Handler for BoxedTextHandler {
    fn document_type(&self) -> DocumentType {
        self.0.document_type()
    }

    fn source(&self) -> ContentSource {
        self.0.source()
    }

    fn encode(&self) -> Result<ContentData, Error> {
        self.0.encode()
    }
}

#[async_trait::async_trait]
impl TextHandler for BoxedTextHandler {
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        self.0.locations()
    }

    async fn read(&self, location: &TextLocation) -> Option<TextData> {
        self.0.read(location).await
    }

    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        self.0.redact_at(location, redaction).await
    }
}

