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
use nvisy_codec::document::{Located, LocationStream};
use nvisy_codec::handler::{
    Handler, ImageData, ImageHandler, ImageRedaction, TextData, TextHandler, TextRedaction,
    apply_text_redaction,
};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::{ImageLocation, TextLocation};
use nvisy_ontology::primitive::Dpi;

use super::pdf_render::PdfRenderer;

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

    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        let offsets = self.page_offsets();
        let Some(page_idx) = offsets.iter().position(|&(start, end, _)| {
            location.start_offset >= start && location.end_offset <= end
        }) else {
            return Ok(());
        };
        let page_start = offsets[page_idx].0;
        let start = location.start_offset - page_start;
        let end = location.end_offset - page_start;

        let mut content = self.pages[page_idx].clone();
        apply_text_redaction(&mut content, &redaction, start, end, TARGET)?;

        if self.document_type == DocumentType::Pdf {
            let mut doc = lopdf::Document::load_mem(&self.raw).map_err(|e| {
                Error::runtime(
                    format!("failed to load PDF for editing: {e}"),
                    TARGET,
                    false,
                )
            })?;
            let old_text = &self.pages[page_idx];
            if !old_text.is_empty() && old_text != &content {
                let _ = doc.replace_text((page_idx as u32) + 1, old_text, &content, None);
            }
            let mut buf = Vec::new();
            doc.save_to(&mut buf).map_err(|e| {
                Error::runtime(format!("failed to save edited PDF: {e}"), TARGET, false)
            })?;
            self.raw = Bytes::from(buf);
        }

        self.pages[page_idx] = content;
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
                        polygon: None,
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

    async fn redact_at(
        &mut self,
        _location: &ImageLocation,
        _redaction: ImageRedaction,
    ) -> Result<(), Error> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl nvisy_codec::handler::RichHandler for RichTextHandler {
    fn text_locations(&self) -> LocationStream<'_, TextLocation> {
        TextHandler::locations(self)
    }

    async fn read_text(&self, location: &TextLocation) -> Option<TextData> {
        TextHandler::read(self, location).await
    }

    async fn redact_text_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        TextHandler::redact_at(self, location, redaction).await
    }

    fn image_locations(&self) -> LocationStream<'_, ImageLocation> {
        ImageHandler::locations(self)
    }

    async fn read_image(&self, location: &ImageLocation) -> Option<ImageData> {
        ImageHandler::read(self, location).await
    }

    async fn redact_image_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error> {
        ImageHandler::redact_at(self, location, redaction).await
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_codec::handler::TextHandler;

    use super::*;

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
}
