//! [`BoxedTabularHandler`]: type-erased wrapper over all tabular handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::TabularLocation;

use super::TabularHandler;
use crate::document::LocationStream;
use crate::handler::{CsvHandler, Handler, TextData, XlsxHandler};
use crate::transform::{Redactions, TabularRedaction};

/// A type-erased tabular handler backed by a boxed trait object.
pub struct BoxedTabularHandler(Box<dyn TabularHandler>);

impl BoxedTabularHandler {
    /// Wrap any concrete tabular handler.
    pub fn new(handler: impl TabularHandler + 'static) -> Self {
        Self(Box::new(handler))
    }
}

impl fmt::Debug for BoxedTabularHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedTabularHandler")
            .field(&self.0.document_type())
            .finish()
    }
}

impl Handler for BoxedTabularHandler {
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
impl TabularHandler for BoxedTabularHandler {
    fn locations(&self) -> LocationStream<'_, TabularLocation> {
        self.0.locations()
    }

    async fn read(&self, location: &TabularLocation) -> Option<TextData> {
        self.0.read(location).await
    }

    async fn redact(
        &mut self,
        redactions: Redactions<TabularLocation, TabularRedaction>,
    ) -> Result<(), Error> {
        self.0.redact(redactions).await
    }
}

impl From<CsvHandler> for BoxedTabularHandler {
    fn from(h: CsvHandler) -> Self {
        Self::new(h)
    }
}

impl From<XlsxHandler> for BoxedTabularHandler {
    fn from(h: XlsxHandler) -> Self {
        Self::new(h)
    }
}
