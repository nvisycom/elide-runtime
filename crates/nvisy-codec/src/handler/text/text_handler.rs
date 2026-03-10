//! [`AnyText`]: type-erased wrapper over all text handler types.

use derive_more::From;
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

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

impl AnyText {
    /// Try to get the inner [`TxtHandler`] by reference.
    pub fn as_txt(&self) -> Option<&TxtHandler> {
        if let Self::Txt(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`TxtHandler`].
    pub fn into_txt(self) -> Option<TxtHandler> {
        if let Self::Txt(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner [`CsvHandler`] by reference.
    pub fn as_csv(&self) -> Option<&CsvHandler> {
        if let Self::Csv(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`CsvHandler`].
    pub fn into_csv(self) -> Option<CsvHandler> {
        if let Self::Csv(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner [`JsonHandler`] by reference.
    pub fn as_json(&self) -> Option<&JsonHandler> {
        if let Self::Json(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`JsonHandler`].
    pub fn into_json(self) -> Option<JsonHandler> {
        if let Self::Json(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner [`HtmlHandler`] by reference.
    #[cfg(feature = "html")]
    pub fn as_html(&self) -> Option<&HtmlHandler> {
        if let Self::Html(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`HtmlHandler`].
    #[cfg(feature = "html")]
    pub fn into_html(self) -> Option<HtmlHandler> {
        if let Self::Html(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner [`XlsxHandler`] by reference.
    #[cfg(feature = "xlsx")]
    pub fn as_xlsx(&self) -> Option<&XlsxHandler> {
        if let Self::Xlsx(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`XlsxHandler`].
    #[cfg(feature = "xlsx")]
    pub fn into_xlsx(self) -> Option<XlsxHandler> {
        if let Self::Xlsx(h) = self {
            Some(h)
        } else {
            None
        }
    }
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
            DocumentType::Text(nvisy_core::fs::TextFormat::Txt),
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
        assert!(txt.as_txt().is_some());
    }

    #[test]
    fn encode_delegates() {
        let h = AnyText::Txt(TxtHandler::new(vec!["hello".into()], false));
        assert!(h.encode().is_ok());
    }
}
