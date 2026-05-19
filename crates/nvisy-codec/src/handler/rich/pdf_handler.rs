//! Rich-text handler: holds per-page extracted text and raw document
//! bytes, providing location-based access via [`Handler`] +
//! [`TextHandler`] + [`ImageHandler`].
//!
//! [`TextHandler::locations`] yields one [`TextLocation`] per page,
//! with byte offsets computed from cumulative page text lengths and
//! `page_number` set. [`ImageHandler::locations`] yields one
//! [`ImageLocation`] per embedded image.
//!
//! [`Handler::encode`] returns the raw document bytes; text redactions
//! applied via [`TextHandler::redact`] are baked into the raw PDF
//! content streams.

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::{ImageLocation, TextLocation};
use nvisy_ontology::primitive::Dpi;

use super::pdf_render::PdfRenderer;
use crate::document::{Located, LocationStream};
use crate::handler::image::ImageData;
use crate::handler::text::TextData;
use crate::handler::{Handler, ImageHandler, TextHandler};
use crate::transform::{
    ImageRedaction, Redactions, TextRedaction, apply_text_redactions,
};

const TARGET: &str = "rich-text-handler";

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
                TARGET,
                false,
            )
        })?;
        if doc.is_encrypted() {
            doc.decrypt(password.unwrap_or("")).map_err(|e| {
                Error::runtime(
                    format!("failed to extract text from PDF: {e}"),
                    TARGET,
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

    /// Total number of pages (alias for [`page_count`]).
    ///
    /// [`page_count`]: Self::page_count
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
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        let source = self.source;
        let items: Vec<_> = self
            .page_offsets()
            .into_iter()
            .map(|(start, end, page)| {
                Located::new(
                    source,
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        page_number: Some(page),
                        ..Default::default()
                    },
                )
            })
            .collect();
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &TextLocation) -> Option<TextData> {
        let offsets = self.page_offsets();
        let page_idx = offsets
            .iter()
            .position(|&(start, _, _)| start == location.start_offset)?;
        self.pages.get(page_idx).cloned().map(TextData::from)
    }

    async fn redact(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        if redactions.is_empty() {
            return Ok(());
        }
        let offsets = self.page_offsets();

        // Compute new page texts by applying redactions to current values.
        let mut page_updates: Vec<(usize, String)> = Vec::new();
        for (loc, items) in redactions {
            let Some(page_idx) = offsets
                .iter()
                .position(|&(start, _, _)| start == loc.start_offset)
            else {
                continue;
            };
            let mut content = self.pages[page_idx].clone();
            apply_text_redactions(&mut content, &items, TARGET)?;
            page_updates.push((page_idx, content));
        }

        // PDF-specific: bake replacements into content streams.
        if self.document_type == DocumentType::Pdf {
            let mut doc = lopdf::Document::load_mem(&self.raw).map_err(|e| {
                Error::runtime(
                    format!("failed to load PDF for editing: {e}"),
                    TARGET,
                    false,
                )
            })?;

            for (idx, new_text) in &page_updates {
                let old_text = &self.pages[*idx];
                if !old_text.is_empty() && old_text != new_text {
                    let _ = doc.replace_text((*idx as u32) + 1, old_text, new_text, None);
                }
            }

            let mut buf = Vec::new();
            doc.save_to(&mut buf).map_err(|e| {
                Error::runtime(format!("failed to save edited PDF: {e}"), TARGET, false)
            })?;
            self.raw = Bytes::from(buf);
        }

        for (idx, new_text) in page_updates {
            self.pages[idx] = new_text;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ImageHandler for RichTextHandler {
    fn locations(&self) -> LocationStream<'_, ImageLocation> {
        let source = self.source;
        let images = match PdfRenderer::extract_images(&self.raw) {
            Ok(imgs) => imgs,
            Err(e) => {
                tracing::warn!(
                    target: "nvisy_codec::rich",
                    error = %e,
                    "failed to extract embedded images",
                );
                return LocationStream::empty();
            }
        };
        let items: Vec<_> = images
            .into_iter()
            .enumerate()
            .map(|(i, _data)| {
                Located::new(
                    source,
                    ImageLocation {
                        bounding_box: nvisy_ontology::primitive::BoundingBox::default(),
                        image_id: None,
                        page_number: Some((i + 1) as u32),
                    },
                )
            })
            .collect();
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, _location: &ImageLocation) -> Option<ImageData> {
        // Cropping embedded PDF images by bounding box is not yet
        // implemented. Requires re-rendering the page region.
        None
    }

    async fn redact(
        &mut self,
        _redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error> {
        Ok(())
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
    async fn locations_yields_one_per_page() {
        let h = handler(&["page one", "page two", "page three"]);
        let items: Vec<_> = TextHandler::locations(&h).collect().await;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].location.page_number, Some(1));
        assert_eq!(items[1].location.page_number, Some(2));
        assert_eq!(items[2].location.page_number, Some(3));
    }

    #[tokio::test]
    async fn locations_empty_document() {
        let h = handler(&[]);
        let items: Vec<_> = TextHandler::locations(&h).collect().await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn read_returns_page_text() {
        let h = handler(&["page one", "page two"]);
        let items: Vec<_> = TextHandler::locations(&h).collect().await;
        assert_eq!(
            TextHandler::read(&h, &items[0].location)
                .await
                .unwrap()
                .as_str(),
            "page one"
        );
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
