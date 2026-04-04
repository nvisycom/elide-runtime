//! [`BoxedTextHandler`]: type-erased wrapper over all text handler types.

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::TextLocation;

use super::TextData;
use crate::document::SpanStream;
use crate::handler::{Handler, TextHandler};

/// A type-erased text handler backed by a boxed trait object.
///
/// Since [`TextHandler`] no longer has an associated type, it is
/// directly object-safe and can be stored as `Box<dyn TextHandler>`.
pub struct BoxedTextHandler(Box<dyn TextHandler>);

impl BoxedTextHandler {
    /// Wrap any concrete text handler.
    pub fn new(handler: impl TextHandler + 'static) -> Self {
        Self(Box::new(handler))
    }
}

impl std::fmt::Debug for BoxedTextHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        self.0.text_spans().await
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        self.0.edit_text(edits).await
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        self.0.value_at(location).await
    }
}

// Explicit From impls for each concrete text handler type.
macro_rules! impl_from_text_handler {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for BoxedTextHandler {
                fn from(h: $ty) -> Self {
                    Self::new(h)
                }
            }
        )+
    };
}

use super::{CsvHandler, JsonHandler, TxtHandler};
impl_from_text_handler!(TxtHandler, CsvHandler, JsonHandler);

#[cfg(feature = "html")]
use super::HtmlHandler;
#[cfg(feature = "html")]
impl_from_text_handler!(HtmlHandler);

#[cfg(feature = "xlsx")]
use super::XlsxHandler;
#[cfg(feature = "xlsx")]
impl_from_text_handler!(XlsxHandler);

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::media::TextFormat;

    use super::*;
    use crate::handler::TxtHandler;

    #[test]
    fn txt_variant_document_type() {
        let h = BoxedTextHandler::from(TxtHandler::new(vec!["hello".into()], false));
        assert_eq!(h.document_type(), DocumentType::Text(TextFormat::Txt));
    }

    #[tokio::test]
    async fn view_spans_returns_text() {
        let h =
            BoxedTextHandler::from(TxtHandler::new(vec!["line1".into(), "line2".into()], false));
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].data, "line1");
        assert_eq!(spans[1].data, "line2");
    }
}
