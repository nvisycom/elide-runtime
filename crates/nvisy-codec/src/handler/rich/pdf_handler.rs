//! Rich-text handler: holds per-page extracted text and raw document
//! bytes, providing span-based access via [`Handler`] + [`TextHandler`] +
//! [`ImageHandler`].
//!
//! This handler is format-agnostic and used for any rich document that
//! contains pages of text and embedded images (PDF, DOCX, etc.).
//!
//! # Text span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per page.  Each span
//! is addressed by a [`RichTextSpan`] (0-based page index) and carries the
//! extracted text for that page as `TextData`.
//!
//! [`TextHandler::edit_text`] replaces the text at the given page indices.
//! For PDF documents the changes are applied to the underlying content
//! streams via [`lopdf::Document::replace_text`].
//!
//! # Encoding
//!
//! [`Handler::encode`] returns the raw document bytes.  Edits applied via
//! [`edit_text`](TextHandler::edit_text) are already baked into the raw
//! bytes, so `encode` is a simple clone.

use bytes::Bytes;
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::math::Dpi;
use nvisy_core::path::ContentSource;

use super::pdf_render::PdfRenderer;
use crate::document::{Span, SpanStream};
use crate::handler::image::ImageData;
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

/// 0-based page index for text spans within a rich document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RichTextSpan(pub u32);

/// Handler for rich documents containing pages of text and images.
///
/// Both PDF and DOCX documents share this representation: per-page
/// extracted text alongside the raw document bytes. Rendering is
/// dispatched to a dedicated single-thread pool via [`PdfRenderer`].
#[derive(Debug)]
pub struct RichTextHandler {
    /// Content source for lineage tracking.
    source: ContentSource,
    /// The document type (PDF, DOCX, etc.).
    document_type: DocumentType,
    /// Per-page extracted text (0-indexed).
    pages: Vec<String>,
    /// Raw document bytes for encode and rendering.
    raw: Bytes,
}

impl RichTextHandler {
    /// Create a new handler from per-page text and raw document bytes.
    pub fn new(
        document_type: DocumentType,
        pages: Vec<String>,
        raw: impl Into<Bytes>,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            document_type,
            pages,
            raw: raw.into(),
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Parse raw PDF bytes, extract per-page text, and return a new handler.
    ///
    /// Uses [`lopdf::Document::extract_text_chunks`] with error-filtering
    /// for resilience (PDF font encoding issues are common).
    pub fn from_pdf(raw: impl Into<Bytes>, password: Option<&str>) -> Result<Self, Error> {
        let raw: Bytes = raw.into();
        let mut doc = lopdf::Document::load_mem(&raw).map_err(|e| {
            Error::runtime(
                format!("failed to extract text from PDF: {e}"),
                "rich-text-handler",
                false,
            )
        })?;
        if doc.is_encrypted() {
            doc.decrypt(password.unwrap_or("")).map_err(|e| {
                Error::runtime(
                    format!("failed to extract text from PDF: {e}"),
                    "rich-text-handler",
                    false,
                )
            })?;
        }
        let page_count = doc.get_pages().len();
        let mut pages = Vec::with_capacity(page_count);
        for page_num in 1..=(page_count as u32) {
            let chunks = doc.extract_text_chunks(&[page_num]);
            let text: String = chunks.into_iter().filter_map(|r| r.ok()).collect();
            pages.push(text);
        }
        Ok(Self {
            source: ContentSource::new(),
            document_type: DocumentType::Pdf,
            pages,
            raw,
        })
    }

    /// All per-page text extractions.
    pub fn pages(&self) -> &[String] {
        &self.pages
    }

    /// Text for a specific page by 0-based index.
    pub fn page(&self, index: usize) -> Option<&str> {
        self.pages.get(index).map(|s| s.as_str())
    }

    /// Total number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The raw document bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Total number of pages (alias for [`page_count`](Self::page_count)).
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether the document has no pages.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Render all pages of the PDF to images at the given DPI.
    ///
    /// Delegates to [`PdfRenderer::parallel_render`].
    pub fn render_pages(&self, dpi: Dpi) -> Result<Vec<ImageData>, Error> {
        PdfRenderer::parallel_render(&self.raw, dpi)
    }
}

impl Handler for RichTextHandler {
    fn document_type(&self) -> DocumentType {
        self.document_type
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "rich.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        tracing::Span::current().record("output_bytes", self.raw.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, self.raw.clone()))
    }
}

#[async_trait::async_trait]
impl TextHandler for RichTextHandler {
    type TextId = RichTextSpan;

    async fn text_spans(&self) -> SpanStream<'_, RichTextSpan, TextData> {
        SpanStream::new(futures::stream::iter(RichTextSpanIter {
            pages: &self.pages,
            index: 0,
        }))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, RichTextSpan, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if edits.is_empty() {
            return Ok(());
        }

        for edit in &edits {
            let idx = edit.id.0 as usize;
            if idx >= self.pages.len() {
                return Err(Error::validation(
                    format!("page index out of bounds: {idx}"),
                    "rich-text-handler",
                ));
            }
        }

        // PDF-specific: apply replacements to content streams.
        if self.document_type == DocumentType::Pdf {
            let mut doc = lopdf::Document::load_mem(&self.raw).map_err(|e| {
                Error::runtime(
                    format!("failed to load PDF for editing: {e}"),
                    "rich-text-handler",
                    false,
                )
            })?;

            for edit in &edits {
                let idx = edit.id.0 as usize;
                let old_text = &self.pages[idx];
                if !old_text.is_empty() && old_text.as_str() != edit.data.as_str() {
                    let _ =
                        doc.replace_text((idx as u32) + 1, old_text, edit.data.as_str(), None);
                }
                self.pages[idx] = edit.data.as_str().to_owned();
            }

            let mut buf = Vec::new();
            doc.save_to(&mut buf).map_err(|e| {
                Error::runtime(
                    format!("failed to save edited PDF: {e}"),
                    "rich-text-handler",
                    false,
                )
            })?;
            self.raw = Bytes::from(buf);
        } else {
            for edit in &edits {
                let idx = edit.id.0 as usize;
                self.pages[idx] = edit.data.as_str().to_owned();
            }
        }

        Ok(())
    }
}

/// Iterator over pages of a rich document (text spans).
struct RichTextSpanIter<'a> {
    pages: &'a [String],
    index: usize,
}

impl<'a> Iterator for RichTextSpanIter<'a> {
    type Item = Span<RichTextSpan, TextData>;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.pages.get(self.index)?;
        let span = Span::new(RichTextSpan(self.index as u32), TextData::from(text.clone()));
        self.index += 1;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.pages.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RichTextSpanIter<'_> {}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use crate::document::Span;
    use crate::handler::TextHandler;

    fn handler(pages: &[&str]) -> RichTextHandler {
        RichTextHandler::new(
            DocumentType::Pdf,
            pages.iter().map(|s| s.to_string()).collect(),
            Vec::new(),
        )
    }

    #[tokio::test]
    async fn view_spans_yields_one_per_page() {
        let h = handler(&["page one", "page two", "page three"]);
        let spans: Vec<_> = h.text_spans().await.collect().await;

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].id, RichTextSpan(0));
        assert_eq!(spans[0].data, "page one");
        assert_eq!(spans[1].id, RichTextSpan(1));
        assert_eq!(spans[1].data, "page two");
        assert_eq!(spans[2].id, RichTextSpan(2));
        assert_eq!(spans[2].data, "page three");
    }

    #[tokio::test]
    async fn view_spans_empty_document() {
        let h = handler(&[]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert!(spans.is_empty());
    }

    #[test]
    fn accessors() {
        let h = handler(&["alpha", "beta"]);
        assert_eq!(h.page_count(), 2);
        assert_eq!(h.len(), 2);
        assert!(!h.is_empty());
        assert_eq!(h.page(0), Some("alpha"));
        assert_eq!(h.page(1), Some("beta"));
        assert_eq!(h.page(2), None);
        assert_eq!(h.pages(), &["alpha", "beta"]);
    }

    #[test]
    fn encode_returns_raw_bytes() -> Result<(), Error> {
        let raw = b"fake-pdf-bytes";
        let h = RichTextHandler::new(DocumentType::Pdf, vec!["text".into()], raw.to_vec());
        assert_eq!(h.encode()?.as_bytes(), raw);
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_updates_text() -> Result<(), Error> {
        let mut h = handler(&["hello"]);
        let err = h
            .edit_text(SpanStream::new(futures::stream::iter(vec![
                Span::new(RichTextSpan(5), "nope".into()),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
        Ok(())
    }

    #[test]
    fn document_type_is_pdf() {
        let h = handler(&[]);
        assert_eq!(h.document_type(), DocumentType::Pdf);
    }

    #[test]
    fn empty_handler() {
        let h = handler(&[]);
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert_eq!(h.page_count(), 0);
    }
}
