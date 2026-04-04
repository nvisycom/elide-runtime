//! Rich-text handler: holds per-page extracted text and raw document
//! bytes, providing span-based access via [`Handler`] + [`TextHandler`] +
//! [`ImageHandler`].
//!
//! # Text span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per page, addressed
//! by [`TextLocation`] with byte offsets computed from cumulative page
//! text lengths and `page_number` set.
//!
//! # Encoding
//!
//! [`Handler::encode`] returns the raw document bytes. Edits applied via
//! [`edit_text`](TextHandler::edit_text) are baked into the raw bytes.

use bytes::Bytes;
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::{ImageLocation, TextLocation};
use nvisy_ontology::math::Dpi;

use super::pdf_render::PdfRenderer;
use crate::document::{Span, SpanStream};
use crate::handler::image::ImageData;
use crate::handler::text::TextData;
use crate::handler::{Handler, ImageHandler, TextHandler};

/// Handler for rich documents containing pages of text and images.
#[derive(Debug)]
pub struct RichTextHandler {
    source: ContentSource,
    document_type: DocumentType,
    pages: Vec<String>,
    raw: Bytes,
}

impl RichTextHandler {
    /// Create a new handler from per-page text and raw document bytes.
    pub fn new(document_type: DocumentType, pages: Vec<String>, raw: impl Into<Bytes>) -> Self {
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
    pub fn render_pages(&self, dpi: Dpi) -> Result<Vec<ImageData>, Error> {
        PdfRenderer::parallel_render(&self.raw, dpi)
    }

    /// Compute `(start_offset, end_offset, page_number)` for each page.
    fn page_offsets(&self) -> Vec<(usize, usize, u32)> {
        let mut offset = 0;
        self.pages
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let start = offset;
                let end = start + text.len();
                offset = end;
                (start, end, (i + 1) as u32)
            })
            .collect()
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
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        let offsets = self.page_offsets();
        let spans: Vec<_> = self
            .pages
            .iter()
            .zip(offsets.iter())
            .map(|(text, &(start, end, page))| {
                Span::new(
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        page_number: Some(page),
                        ..Default::default()
                    },
                    TextData::from(text.clone()),
                )
                .with_source(self.source)
            })
            .collect();
        SpanStream::new(futures::stream::iter(spans))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if edits.is_empty() {
            return Ok(());
        }

        let offsets = self.page_offsets();

        // Map byte offsets to page indices.
        let mut page_edits: Vec<(usize, String)> = Vec::new();
        for edit in &edits {
            let page_idx = offsets
                .iter()
                .position(|&(start, _, _)| start == edit.id.start_offset)
                .ok_or_else(|| {
                    Error::validation(
                        format!("no page at byte offset {}", edit.id.start_offset),
                        "rich-text-handler",
                    )
                })?;
            page_edits.push((page_idx, edit.data.as_str().to_owned()));
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

            for &(idx, ref new_text) in &page_edits {
                let old_text = &self.pages[idx];
                if !old_text.is_empty() && old_text != new_text {
                    let _ = doc.replace_text((idx as u32) + 1, old_text, new_text, None);
                }
                self.pages[idx] = new_text.clone();
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
            for (idx, new_text) in page_edits {
                self.pages[idx] = new_text;
            }
        }

        Ok(())
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        let offsets = self.page_offsets();
        let page_idx = offsets
            .iter()
            .position(|&(start, _, _)| start == location.start_offset)?;
        self.pages.get(page_idx).cloned()
    }
}

#[async_trait::async_trait]
impl ImageHandler for RichTextHandler {
    async fn image_spans(&self) -> SpanStream<'_, ImageLocation, ImageData> {
        let images = match PdfRenderer::extract_images(&self.raw) {
            Ok(imgs) => imgs,
            Err(e) => {
                tracing::warn!(
                    target: "nvisy_codec::rich",
                    error = %e,
                    "failed to extract embedded images",
                );
                return SpanStream::new(futures::stream::empty());
            }
        };
        SpanStream::new(futures::stream::iter(
            images.into_iter().enumerate().map(|(i, data)| {
                // Embedded image bounding box — exact position within
                // the page requires PDF content stream parsing. For now,
                // use a full-page placeholder that identifies the page.
                let location = ImageLocation {
                    bounding_box: nvisy_ontology::math::BoundingBox::default(),
                    value: None,
                    image_id: None,
                    page_number: Some((i + 1) as u32),
                };
                Span::new(location, data)
            }),
        ))
    }

    async fn edit_images(
        &mut self,
        _edits: SpanStream<'_, ImageLocation, ImageData>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn value_at(&self, _location: &ImageLocation) -> Option<ImageData> {
        // Cropping embedded PDF images by bounding box is not yet
        // implemented. Requires re-rendering the page region.
        None
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
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
        assert_eq!(spans[0].id.page_number, Some(1));
        assert_eq!(spans[0].data, "page one");
        assert_eq!(spans[1].id.page_number, Some(2));
        assert_eq!(spans[1].data, "page two");
        assert_eq!(spans[2].id.page_number, Some(3));
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
