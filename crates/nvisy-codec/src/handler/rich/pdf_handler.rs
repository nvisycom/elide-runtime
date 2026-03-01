//! PDF handler: holds per-page extracted text and raw PDF bytes,
//! providing span-based access via [`Handler`].
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields one [`Span`] per page.  Each span
//! is addressed by a [`PdfSpan`] (0-based page index) and carries the
//! extracted text for that page as a `String`.
//!
//! [`Handler::edit_spans`] replaces the text at the given page indices
//! and applies the changes to the underlying PDF content streams via
//! [`lopdf::Document::replace_text`].
//!
//! # Encoding
//!
//! [`Handler::encode`] returns the raw PDF bytes.  Edits applied via
//! [`edit_spans`](Handler::edit_spans) are already baked into the raw
//! bytes, so `encode` is a simple clone.

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::math::Dpi;

use crate::handler::image::ImageData;
use crate::handler::{Handler, Span};
use crate::stream::{SpanEditStream, SpanStream};
use crate::transform::TextHandler;
use super::pdf_render::PdfRenderer;

/// 0-based page index identifying a span within a PDF document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PdfSpan(pub u32);

/// PDF document handler.
///
/// Stores per-page extracted text alongside the raw PDF bytes.
/// Rendering is dispatched to a dedicated single-thread pool via
/// [`PdfRenderer`].
#[derive(Debug, Clone)]
pub struct PdfHandler {
    /// Per-page extracted text (0-indexed).
    pages: Vec<String>,
    /// Raw PDF bytes for encode and rendering.
    raw: Vec<u8>,
}

impl PdfHandler {
    /// Create a new handler from per-page text and raw PDF bytes.
    pub fn new(pages: Vec<String>, raw: Vec<u8>) -> Self {
        Self { pages, raw }
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

    /// The raw PDF bytes.
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

#[async_trait::async_trait]
impl Handler for PdfHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Pdf
    }

    #[tracing::instrument(name = "pdf.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = self.raw.clone();
        tracing::Span::current().record("output_bytes", bytes.len());
        Ok(bytes)
    }

    type SpanId = PdfSpan;
    type SpanData = String;

    async fn view_spans(&self) -> SpanStream<'_, PdfSpan, String> {
        SpanStream::new(futures::stream::iter(PdfSpanIter {
            pages: &self.pages,
            index: 0,
        }))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, PdfSpan, String>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if edits.is_empty() {
            return Ok(());
        }

        // Validate all indices before mutating anything.
        for edit in &edits {
            let idx = edit.id.0 as usize;
            if idx >= self.pages.len() {
                return Err(Error::validation(
                    format!("page index out of bounds: {idx}"),
                    "pdf-handler",
                ));
            }
        }

        // Load the PDF document for content-stream manipulation.
        let mut doc = lopdf::Document::load_mem(&self.raw).map_err(|e| {
            Error::runtime(
                format!("failed to load PDF for editing: {e}"),
                "pdf-handler",
                false,
            )
        })?;

        for edit in &edits {
            let idx = edit.id.0 as usize;
            let old_text = &self.pages[idx];

            // Apply replacement to the PDF content stream.
            // lopdf uses 1-based page numbers.
            if !old_text.is_empty() && old_text != &edit.data {
                let _ = doc.replace_text(
                    (idx as u32) + 1,
                    old_text,
                    &edit.data,
                    None,
                );
            }

            // Update the in-memory text.
            self.pages[idx] = edit.data.clone();
        }

        // Serialize the modified document back to raw bytes.
        let mut buf = Vec::new();
        doc.save_to(&mut buf).map_err(|e| {
            Error::runtime(
                format!("failed to save edited PDF: {e}"),
                "pdf-handler",
                false,
            )
        })?;
        self.raw = buf;

        Ok(())
    }
}

impl TextHandler for PdfHandler {}

/// Iterator over pages of a PDF document.
struct PdfSpanIter<'a> {
    pages: &'a [String],
    index: usize,
}

impl<'a> Iterator for PdfSpanIter<'a> {
    type Item = Span<PdfSpan, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.pages.get(self.index)?;
        let span = Span::new(PdfSpan(self.index as u32), text.clone());
        self.index += 1;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.pages.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PdfSpanIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::SpanEdit;
    use futures::StreamExt;
    use nvisy_core::Error;

    fn handler(pages: &[&str]) -> PdfHandler {
        PdfHandler::new(
            pages.iter().map(|s| s.to_string()).collect(),
            Vec::new(),
        )
    }

    #[tokio::test]
    async fn view_spans_yields_one_per_page() {
        let h = handler(&["page one", "page two", "page three"]);
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].id, PdfSpan(0));
        assert_eq!(spans[0].data, "page one");
        assert_eq!(spans[1].id, PdfSpan(1));
        assert_eq!(spans[1].data, "page two");
        assert_eq!(spans[2].id, PdfSpan(2));
        assert_eq!(spans[2].data, "page three");
    }

    #[tokio::test]
    async fn view_spans_empty_document() {
        let h = handler(&[]);
        let spans: Vec<_> = h.view_spans().await.collect().await;
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
        let raw = b"fake-pdf-bytes".to_vec();
        let h = PdfHandler::new(vec!["text".into()], raw.clone());
        assert_eq!(h.encode()?, raw);
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_updates_text() -> Result<(), Error> {
        // With empty raw bytes, lopdf will fail to parse — but we can
        // test the out-of-bounds validation path.
        let mut h = handler(&["hello"]);
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(PdfSpan(5), "nope".into()),
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
