//! [`AnyRich`]: type-erased wrapper over all rich-document handler types.

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

#[cfg(feature = "docx")]
use super::DocxHandler;
#[cfg(feature = "pdf")]
use super::PdfHandler;
use crate::document::SpanStream;
use crate::handler::text::{TextData, forward_edits, reindex_stream};
use crate::handler::{Handler, TextHandler};

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
        if let Self::Pdf(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`PdfHandler`].
    #[cfg(feature = "pdf")]
    pub fn into_pdf(self) -> Option<PdfHandler> {
        if let Self::Pdf(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner [`DocxHandler`] by reference.
    #[cfg(feature = "docx")]
    pub fn as_docx(&self) -> Option<&DocxHandler> {
        if let Self::Docx(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`DocxHandler`].
    #[cfg(feature = "docx")]
    pub fn into_docx(self) -> Option<DocxHandler> {
        if let Self::Docx(h) = self {
            Some(h)
        } else {
            None
        }
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

    fn source(&self) -> ContentSource {
        match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => h.source(),
            #[cfg(feature = "docx")]
            Self::Docx(h) => h.source(),
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

#[async_trait::async_trait]
impl TextHandler for AnyRich {
    type TextId = usize;

    async fn text_spans(&self) -> SpanStream<'_, usize, TextData> {
        match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => reindex_stream(h).await,
            #[cfg(feature = "docx")]
            Self::Docx(h) => reindex_stream(h).await,
        }
    }

    async fn edit_text(&mut self, edits: SpanStream<'_, usize, TextData>) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        match self {
            #[cfg(feature = "pdf")]
            Self::Pdf(h) => forward_edits(h, edits).await,
            #[cfg(feature = "docx")]
            Self::Docx(h) => forward_edits(h, edits).await,
        }
    }
}
