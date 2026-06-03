//! Rich-text handler: holds per-page extracted text and raw document
//! bytes, providing location-based access via [`Handler`] +
//! `Handle<Text>` + `Handle<Image>`.
//!
//! [`Handle<Text>::locations`] yields one [`Text`] per page, with byte
//! offsets computed from cumulative page text lengths and
//! `page_number` set. [`Handle<Image>::locations`] yields one
//! [`Image`] per embedded image.
//!
//! [`Handler::encode`] returns the raw document bytes; text redactions
//! applied via [`Handle<Text>::redact`] are baked into the raw PDF
//! content streams.
//!
//! [`Handle<Text>::locations`]: nvisy_codec::core::Handle::locations
//! [`Handle<Image>::locations`]: nvisy_codec::core::Handle::locations
//! [`Handle<Text>::redact`]: nvisy_codec::core::Handle::redact

use bytes::Bytes;
use nvisy_codec::core::{Handle, Located, LocationStream};
use nvisy_codec::handler::{Handler, ImageData, ImageRedaction, TextData, TextRedaction};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, DocumentType};
use nvisy_core::modality::{Image, ImageLocation, Text, TextLocation};
use nvisy_core::primitive::{BoundingBox, Dpi};

use super::pdf_render::PdfRenderer;
use crate::text::redact;

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

    /// The raw document bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Render all pages of the PDF to images at the given DPI.
    pub fn render_pages(&self, dpi: Dpi) -> Result<Vec<ImageData>, Error> {
        PdfRenderer::parallel_render(&self.raw, dpi)
    }

    /// Compute `(start, end, page_number)` for each page.
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
impl Handle<Text> for RichTextHandler {
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        let source = self.source;
        let items: Vec<_> = self
            .page_offsets()
            .into_iter()
            .map(|(start, end, page)| {
                Located::new(
                    source,
                    TextLocation {
                        start,
                        end,
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
            .position(|&(start, _, _)| start == location.start)?;
        self.pages.get(page_idx).cloned().map(TextData::from)
    }

    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        let offsets = self.page_offsets();
        let Some(page_idx) = offsets
            .iter()
            .position(|&(start, end, _)| location.start >= start && location.end <= end)
        else {
            return Ok(());
        };
        let page_start = offsets[page_idx].0;
        let start = location.start - page_start;
        let end = location.end - page_start;

        let mut content = self.pages[page_idx].clone();
        let value = redaction.output().replacement_value().unwrap_or_default();
        redact::replace_range(&mut content, value, start, end, TARGET)?;

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
impl Handle<Image> for RichTextHandler {
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
                        bounding_box: BoundingBox::default(),
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

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_codec::core::Handle;
    use nvisy_core::modality::Text;

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
        let items: Vec<_> = <RichTextHandler as Handle<Text>>::locations(&h)
            .collect()
            .await;
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].location.page_number, Some(1));
        assert_eq!(items[1].location.page_number, Some(2));
        assert_eq!(items[2].location.page_number, Some(3));
    }

    #[tokio::test]
    async fn read_returns_page_text() {
        let h = handler(&["page one", "page two"]);
        let items: Vec<_> = <RichTextHandler as Handle<Text>>::locations(&h)
            .collect()
            .await;
        assert_eq!(
            <RichTextHandler as Handle<Text>>::read(&h, &items[0].location)
                .await
                .unwrap()
                .as_str(),
            "page one"
        );
    }
}
