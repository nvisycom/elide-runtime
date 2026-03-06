//! [`AnyRich`]: type-erased wrapper over all rich-document handler types.

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use crate::document::span::Span;
use crate::handler::text::TextData;
use crate::handler::{Handler, SpanEdit, SpanEditStream, SpanStream, TextHandler};

#[cfg(feature = "pdf")]
use super::PdfHandler;
#[cfg(feature = "docx")]
use super::DocxHandler;

/// A type-erased rich-document handler that can hold any supported rich format.
///
/// Like [`AnyText`](crate::handler::AnyText), uses `TextId = usize` as a
/// positional span index to unify the heterogeneous span identifiers.
#[derive(Debug)]
pub enum AnyRich {
    #[cfg(feature = "pdf")]
    Pdf(PdfHandler),
    #[cfg(feature = "docx")]
    Docx(DocxHandler),
}

#[cfg(feature = "pdf")]
impl From<PdfHandler> for AnyRich {
    fn from(h: PdfHandler) -> Self {
        Self::Pdf(h)
    }
}

#[cfg(feature = "docx")]
impl From<DocxHandler> for AnyRich {
    fn from(h: DocxHandler) -> Self {
        Self::Docx(h)
    }
}

impl AnyRich {
    /// Try to get the inner [`PdfHandler`] by reference.
    #[cfg(feature = "pdf")]
    pub fn as_pdf(&self) -> Option<&PdfHandler> {
        if let Self::Pdf(h) = self { Some(h) } else { None }
    }

    /// Consume and return the inner [`PdfHandler`].
    #[cfg(feature = "pdf")]
    pub fn into_pdf(self) -> Option<PdfHandler> {
        if let Self::Pdf(h) = self { Some(h) } else { None }
    }

    /// Try to get the inner [`DocxHandler`] by reference.
    #[cfg(feature = "docx")]
    pub fn as_docx(&self) -> Option<&DocxHandler> {
        if let Self::Docx(h) = self { Some(h) } else { None }
    }

    /// Consume and return the inner [`DocxHandler`].
    #[cfg(feature = "docx")]
    pub fn into_docx(self) -> Option<DocxHandler> {
        if let Self::Docx(h) = self { Some(h) } else { None }
    }
}

impl Handler for AnyRich {
    fn document_type(&self) -> DocumentType {
        match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => h.document_type(),
            #[cfg(feature = "docx")]
            Self::Docx(h) => h.document_type(),
        }
    }

    fn encode(&self) -> Result<ContentData, Error> {
        match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => h.encode(),
            #[cfg(feature = "docx")]
            Self::Docx(h) => h.encode(),
        }
    }
}

/// Collect all spans from a handler, re-indexing with `usize`.
async fn reindex_spans<H: TextHandler>(handler: &H) -> Vec<Span<usize, TextData>> {
    handler
        .text_spans()
        .await
        .enumerate()
        .map(|(i, s)| Span::new(i, s.data).with_source(s.source))
        .collect()
        .await
}

/// Collect span IDs from a handler so we can map usize back to the native ID.
async fn collect_ids<H: TextHandler>(handler: &H) -> Vec<H::TextId> {
    handler.text_spans().await.map(|s| s.id).collect().await
}

/// Map edits from `usize` indices back to native IDs and forward them.
async fn forward_edits<H: TextHandler>(
    handler: &mut H,
    edits: Vec<SpanEdit<usize, TextData>>,
) -> Result<(), Error> {
    let ids = collect_ids(handler).await;
    let mapped: Vec<_> = edits
        .into_iter()
        .filter_map(|e| ids.get(e.id).cloned().map(|id| SpanEdit::new(id, e.data)))
        .collect();
    handler
        .edit_text(SpanEditStream::new(futures::stream::iter(mapped)))
        .await
}

#[async_trait::async_trait]
impl TextHandler for AnyRich {
    type TextId = usize;

    async fn text_spans(&self) -> SpanStream<'_, usize, TextData> {
        let spans = match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => reindex_spans(h).await,
            #[cfg(feature = "docx")]
            Self::Docx(h) => reindex_spans(h).await,
        };
        SpanStream::new(futures::stream::iter(spans))
    }

    async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, usize, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => forward_edits(h, edits).await,
            #[cfg(feature = "docx")]
            Self::Docx(h) => forward_edits(h, edits).await,
        }
    }
}
