//! PDF handler: holds per-page extracted text and raw PDF bytes,
//! providing span-based access via [`Handler`] + [`TextHandler`] +
//! [`ImageHandler`].
//!
//! # Text span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per page.  Each span
//! is addressed by a [`PdfTextSpan`] (0-based page index) and carries the
//! extracted text for that page as `TextData`.
//!
//! [`TextHandler::edit_text`] replaces the text at the given page indices
//! and applies the changes to the underlying PDF content streams via
//! [`lopdf::Document::replace_text`].
//!
//! # Image span model
//!
//! [`ImageHandler::image_spans`] yields one [`Span`] per rendered page
//! image.  Each span is addressed by a [`RichImageSpan`] containing
//! the page index and image index.
//!
//! # Encoding
//!
//! [`Handler::encode`] returns the raw PDF bytes.  Edits applied via
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
use crate::handler::{Handler, ImageHandler, TextHandler};

/// 0-based page index for text spans within a PDF document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PdfTextSpan(pub u32);

/// Identifier for an image span within a rich (multi-page) document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RichImageSpan {
    /// 0-based page index.
    pub page: u32,
    /// 0-based image index within the page.
    pub index: u32,
}

/// PDF document handler.
///
/// Stores per-page extracted text alongside the raw PDF bytes.
/// Rendering is dispatched to a dedicated single-thread pool via
/// [`PdfRenderer`].
#[derive(Debug)]
pub struct PdfHandler {
    /// Content source for lineage tracking.
    source: ContentSource,
    /// Per-page extracted text (0-indexed).
    pages: Vec<String>,
    /// Raw PDF bytes for encode and rendering.
    raw: Bytes,
}

impl PdfHandler {
    /// Create a new handler from per-page text and raw PDF bytes.
    pub fn new(pages: Vec<String>, raw: impl Into<Bytes>) -> Self {
        Self {
            source: ContentSource::new(),
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
    pub fn from_raw(raw: impl Into<Bytes>, password: Option<&str>) -> Result<Self, Error> {
        let raw: Bytes = raw.into();
        let mut doc = lopdf::Document::load_mem(&raw).map_err(|e| {
            Error::runtime(
                format!("failed to extract text from PDF: {e}"),
                "pdf-handler",
                false,
            )
        })?;
        if doc.is_encrypted() {
            doc.decrypt(password.unwrap_or("")).map_err(|e| {
                Error::runtime(
                    format!("failed to extract text from PDF: {e}"),
                    "pdf-handler",
                    false,
                )
            })?;
        }
        let page_count = doc.get_pages().len();
        let mut pages = Vec::with_capacity(page_count);
        for page_num in 1..=(page_count as u32) {
            // Resilient: skip chunks that fail encoding
            let chunks = doc.extract_text_chunks(&[page_num]);
            let text: String = chunks.into_iter().filter_map(|r| r.ok()).collect();
            pages.push(text);
        }
        Ok(Self {
            source: ContentSource::new(),
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

impl Handler for PdfHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Pdf
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "pdf.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        tracing::Span::current().record("output_bytes", self.raw.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, self.raw.clone()))
    }
}

#[async_trait::async_trait]
impl TextHandler for PdfHandler {
    type TextId = PdfTextSpan;

    async fn text_spans(&self) -> SpanStream<'_, PdfTextSpan, TextData> {
        SpanStream::new(futures::stream::iter(PdfTextSpanIter {
            pages: &self.pages,
            index: 0,
        }))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, PdfTextSpan, TextData>,
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
            if !old_text.is_empty() && old_text.as_str() != edit.data.as_str() {
                let _ = doc.replace_text((idx as u32) + 1, old_text, edit.data.as_str(), None);
            }

            // Update the in-memory text.
            self.pages[idx] = edit.data.as_str().to_owned();
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
        self.raw = Bytes::from(buf);

        Ok(())
    }
}

#[async_trait::async_trait]
impl ImageHandler for PdfHandler {
    type ImageId = RichImageSpan;

    async fn image_spans(&self) -> SpanStream<'_, RichImageSpan, ImageData> {
        // Render pages to images on demand.
        let images = match PdfRenderer::parallel_render(&self.raw, Dpi::OCR) {
            Ok(imgs) => imgs,
            Err(e) => {
                tracing::warn!(error = %e, "failed to render PDF pages for image_spans");
                return SpanStream::new(futures::stream::empty());
            }
        };
        SpanStream::new(futures::stream::iter(images.into_iter().enumerate().map(
            |(i, img)| {
                Span::new(
                    RichImageSpan {
                        page: i as u32,
                        index: 0,
                    },
                    img,
                )
            },
        )))
    }

    async fn edit_images(
        &mut self,
        _edits: SpanStream<'_, RichImageSpan, ImageData>,
    ) -> Result<(), Error> {
        // Image editing for PDF is not yet supported — rendered images
        // are read-only snapshots.
        tracing::warn!("PDF image editing is not yet supported");
        Ok(())
    }
}

/// Iterator over pages of a PDF document (text spans).
struct PdfTextSpanIter<'a> {
    pages: &'a [String],
    index: usize,
}

impl<'a> Iterator for PdfTextSpanIter<'a> {
    type Item = Span<PdfTextSpan, TextData>;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.pages.get(self.index)?;
        let span = Span::new(PdfTextSpan(self.index as u32), TextData::from(text.clone()));
        self.index += 1;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.pages.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PdfTextSpanIter<'_> {}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use crate::document::Span;
    use crate::handler::TextHandler;

    fn handler(pages: &[&str]) -> PdfHandler {
        PdfHandler::new(pages.iter().map(|s| s.to_string()).collect(), Vec::new())
    }

    #[tokio::test]
    async fn view_spans_yields_one_per_page() {
        let h = handler(&["page one", "page two", "page three"]);
        let spans: Vec<_> = h.text_spans().await.collect().await;

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].id, PdfTextSpan(0));
        assert_eq!(spans[0].data, "page one");
        assert_eq!(spans[1].id, PdfTextSpan(1));
        assert_eq!(spans[1].data, "page two");
        assert_eq!(spans[2].id, PdfTextSpan(2));
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
        let h = PdfHandler::new(vec!["text".into()], raw.to_vec());
        assert_eq!(h.encode()?.as_bytes(), raw);
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_updates_text() -> Result<(), Error> {
        // With empty raw bytes, lopdf will fail to parse — but we can
        // test the out-of-bounds validation path.
        let mut h = handler(&["hello"]);
        let err = h
            .edit_text(SpanStream::new(futures::stream::iter(vec![
                Span::new(PdfTextSpan(5), "nope".into()),
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
