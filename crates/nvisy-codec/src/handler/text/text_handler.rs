//! [`AnyText`]: type-erased wrapper over all text handler types.

use derive_more::From;
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::media::DocumentType;
use nvisy_core::content::ContentData;
use nvisy_core::content::ContentSource;

#[cfg(feature = "html")]
use super::HtmlHandler;
#[cfg(feature = "xlsx")]
use super::XlsxHandler;
use super::{CsvHandler, JsonHandler, TextData, TxtHandler, forward_edits, reindex_stream};
use crate::document::SpanStream;
use crate::handler::{Handler, TextHandler};

/// A type-erased text handler that can hold any supported text format.
///
/// Since different text handlers use different span identifiers, `AnyText`
/// uses `TextId = usize` as a positional span index.
#[derive(Debug, From)]
pub enum AnyText {
    Txt(TxtHandler),
    Csv(CsvHandler),
    Json(JsonHandler),
    #[cfg(feature = "html")]
    Html(HtmlHandler),
    #[cfg(feature = "xlsx")]
    Xlsx(XlsxHandler),
}

impl Handler for AnyText {
    fn document_type(&self) -> DocumentType {
        match self {
            Self::Txt(h) => h.document_type(),
            Self::Csv(h) => h.document_type(),
            Self::Json(h) => h.document_type(),
            #[cfg(feature = "html")]
            Self::Html(h) => h.document_type(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => h.document_type(),
        }
    }

    fn source(&self) -> ContentSource {
        match self {
            Self::Txt(h) => h.source(),
            Self::Csv(h) => h.source(),
            Self::Json(h) => h.source(),
            #[cfg(feature = "html")]
            Self::Html(h) => h.source(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => h.source(),
        }
    }

    fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Txt(h) => h.encode(),
            Self::Csv(h) => h.encode(),
            Self::Json(h) => h.encode(),
            #[cfg(feature = "html")]
            Self::Html(h) => h.encode(),
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => h.encode(),
        }
    }
}

#[async_trait::async_trait]
impl TextHandler for AnyText {
    type TextId = usize;

    async fn text_spans(&self) -> SpanStream<'_, usize, TextData> {
        match self {
            Self::Txt(h) => reindex_stream(h).await,
            Self::Csv(h) => reindex_stream(h).await,
            Self::Json(h) => reindex_stream(h).await,
            #[cfg(feature = "html")]
            Self::Html(h) => reindex_stream(h).await,
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => reindex_stream(h).await,
        }
    }

    async fn edit_text(&mut self, edits: SpanStream<'_, usize, TextData>) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        match self {
            Self::Txt(h) => forward_edits(h, edits).await,
            Self::Csv(h) => forward_edits(h, edits).await,
            Self::Json(h) => forward_edits(h, edits).await,
            #[cfg(feature = "html")]
            Self::Html(h) => forward_edits(h, edits).await,
            #[cfg(feature = "xlsx")]
            Self::Xlsx(h) => forward_edits(h, edits).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[test]
    fn txt_variant_document_type() {
        let h = AnyText::Txt(TxtHandler::new(vec!["hello".into()], false));
        assert_eq!(
            h.document_type(),
            DocumentType::Text(nvisy_core::media::TextFormat::Txt),
        );
    }

    #[tokio::test]
    async fn view_spans_returns_text() {
        let h = AnyText::Txt(TxtHandler::new(vec!["line1".into(), "line2".into()], false));
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].id, 0);
        assert_eq!(spans[1].id, 1);
    }

    #[test]
    fn from_conversions() {
        let txt: AnyText = TxtHandler::new(vec![], false).into();
        assert!(matches!(txt, AnyText::Txt(_)));
    }

    #[test]
    fn encode_delegates() {
        let h = AnyText::Txt(TxtHandler::new(vec!["hello".into()], false));
        assert!(h.encode().is_ok());
    }
}
