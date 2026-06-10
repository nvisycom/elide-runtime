//! PDF handler: holds per-page extracted text and raw document bytes,
//! exposing per-page chunks via [`Handle<Text>`].
//!
//! Text offsets are cumulative over the per-page text sequence in
//! document order. Each chunk carries the page number on its
//! [`TextLocation`] for downstream provenance.
//!
//! Embedded image extraction (figures, page rasterization for OCR)
//! lives on inherent methods rather than `Handle<Image>` — see
//! [`render_pages`] and [`extract_embedded_images`]. These
//! return [`DocumentHandle<Image>`] values backed by PNG bytes so
//! downstream extractors can route them through the standard image
//! pipeline.
//!
//! [`Handle<Text>`]: crate::core::Handle
//! [`render_pages`]: PdfHandler::render_pages
//! [`extract_embedded_images`]: PdfHandler::extract_embedded_images

use std::ops::Range;
use std::sync::Arc;

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::modality::{Image, Text, TextData, TextLocation};
use nvisy_core::primitive::Dpi;
use nvisy_core::redaction::{Redactions, TextReplacement};

use super::PdfLoader;
use super::pdf_render::PdfRenderer;
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handle, Handler, ModalityKind};
use crate::handler::image::PngHandler;
use crate::handler::text::{lift_identity, redact};
use crate::{DocumentHandle, Format, FormatId, LoaderAdapter};

const TARGET: &str = "pdf-handler";

/// Stable [`FormatId`] for the PDF codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.rich.pdf");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Text,
        extensions: vec!["pdf".into()],
        content_types: vec!["application/pdf".into()],
        loader: Arc::new(LoaderAdapter::new(PdfLoader::default())),
    }
}

/// Handler for loaded PDF content.
///
/// `page_starts` is a cumulative-offset index over the per-page text
/// sequence: `page_starts[i]` is the byte position of page `i+1`'s
/// text. Maintained on every redaction; resolves `(byte offset →
/// page index)` in `O(log N)`.
#[derive(Debug)]
pub struct PdfHandler {
    source: ContentSource,
    pages: Vec<String>,
    page_starts: Vec<usize>,
    raw: Bytes,
    cursor: usize,
}

impl PdfHandler {
    /// Create a handler from per-page text and raw PDF bytes.
    pub fn new(pages: Vec<String>, raw: impl Into<Bytes>) -> Self {
        let page_starts = compute_page_starts(&pages);
        Self {
            source: ContentSource::new(),
            pages,
            page_starts,
            raw: raw.into(),
            cursor: 0,
        }
    }

    /// Attach a content source for lineage tracking.
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
        Ok(Self::new(pages, raw))
    }

    /// The raw PDF bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Number of pages.
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether the document has no pages.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Rewind the streaming cursor to the first page.
    pub fn rewind(&mut self) {
        self.cursor = 0;
    }

    /// Render all pages as PNG-backed [`DocumentHandle<Image>`]s at
    /// the given DPI. Used by the OCR fallback when text extraction
    /// yields nothing.
    pub fn render_pages(&self, dpi: Dpi) -> Result<Vec<DocumentHandle<Image>>, Error> {
        let images = PdfRenderer::parallel_render(&self.raw, dpi)?;
        Ok(images.into_iter().map(wrap_as_image_handle).collect())
    }

    /// Extract embedded image objects (figures, photos) from the PDF
    /// content streams. Returns each as a PNG-backed
    /// [`DocumentHandle<Image>`].
    pub fn extract_embedded_images(&self) -> Result<Vec<DocumentHandle<Image>>, Error> {
        let images = PdfRenderer::extract_images(&self.raw)?;
        Ok(images.into_iter().map(wrap_as_image_handle).collect())
    }

    fn page_for(&self, byte_offset: usize) -> Option<usize> {
        match self.page_starts.binary_search(&byte_offset) {
            Ok(i) if i < self.pages.len() => Some(i),
            Ok(_) => None,
            Err(i) if i > 0 && i <= self.pages.len() => Some(i - 1),
            _ => None,
        }
    }

    fn shift_starts_after(&mut self, i: usize, delta: isize) {
        if delta == 0 {
            return;
        }
        for s in &mut self.page_starts[i + 1..] {
            *s = (*s as isize + delta) as usize;
        }
    }
}

impl Handler for PdfHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
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
impl Handle<Text> for PdfHandler {
    async fn next_chunk(&mut self) -> Result<Option<Chunk<Text>>, Error> {
        if self.cursor >= self.pages.len() {
            return Ok(None);
        }
        let i = self.cursor;
        let start = self.page_starts[i];
        let end = self.page_starts[i + 1];
        let text = &self.pages[i];
        self.cursor += 1;
        Ok(Some(Chunk {
            location: TextLocation {
                start,
                end,
                page_number: Some((i + 1) as u32),
                ..Default::default()
            },
            data: TextData::from(text.as_str()),
            embed: None,
        }))
    }

    fn lift_chunk(&self, chunk: &Chunk<Text>, value_range: Range<usize>) -> Option<TextLocation> {
        lift_identity(chunk, value_range)
    }

    async fn read(&self, location: &TextLocation) -> Result<Option<TextData>, Error> {
        let Some(i) = self.page_for(location.start) else {
            return Ok(None);
        };
        let page_start = self.page_starts[i];
        let page_end = self.page_starts[i + 1];
        if location.end > page_end {
            return Ok(None);
        }
        let local_start = location.start - page_start;
        let local_end = location.end - page_start;
        Ok(self.pages[i]
            .get(local_start..local_end)
            .map(TextData::from))
    }

    async fn redact(&mut self, redactions: Redactions<Text>) -> Result<(), Error> {
        // Right-to-left so each page's length delta doesn't invalidate
        // earlier byte offsets.
        let mut items = redactions.into_items();
        items.sort_by_key(|(loc, _)| std::cmp::Reverse(loc.start));
        for (location, replacement) in items {
            self.redact_one(&location, replacement)?;
        }
        Ok(())
    }
}

impl PdfHandler {
    fn redact_one(
        &mut self,
        location: &TextLocation,
        replacement: TextReplacement,
    ) -> Result<(), Error> {
        let Some(i) = self.page_for(location.start) else {
            return Ok(());
        };
        let page_start = self.page_starts[i];
        let page_end = self.page_starts[i + 1];
        if location.end > page_end {
            return Ok(());
        }
        let local_start = location.start - page_start;
        let local_end = location.end - page_start;

        let mut content = self.pages[i].clone();
        let value = replacement.replacement_value().unwrap_or_default();
        let before_len = self.pages[i].len();
        redact::replace_range(&mut content, value, local_start, local_end, TARGET)?;

        // Bake the edit into the raw PDF content stream so the encoded
        // bytes round-trip with the redaction.
        let old_text = &self.pages[i];
        if !old_text.is_empty() && old_text != &content {
            let mut doc = lopdf::Document::load_mem(&self.raw).map_err(|e| {
                Error::runtime(
                    format!("failed to load PDF for editing: {e}"),
                    TARGET,
                    false,
                )
            })?;
            let _ = doc.replace_text((i as u32) + 1, old_text, &content, None);
            let mut buf = Vec::new();
            doc.save_to(&mut buf).map_err(|e| {
                Error::runtime(format!("failed to save edited PDF: {e}"), TARGET, false)
            })?;
            self.raw = Bytes::from(buf);
        }

        self.pages[i] = content;
        let delta = self.pages[i].len() as isize - before_len as isize;
        self.shift_starts_after(i, delta);
        Ok(())
    }
}

fn compute_page_starts(pages: &[String]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(pages.len() + 1);
    let mut offset = 0usize;
    for page in pages {
        starts.push(offset);
        offset += page.len();
    }
    starts.push(offset);
    starts
}

/// Wrap a decoded [`image::DynamicImage`] into a PNG-backed
/// [`DocumentHandle<Image>`].
fn wrap_as_image_handle(img: image::DynamicImage) -> DocumentHandle<Image> {
    let handler = PngHandler::new(img);
    let format = handler.format();
    DocumentHandle::new(format, Box::new(handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(pages: &[&str]) -> PdfHandler {
        PdfHandler::new(pages.iter().map(|s| s.to_string()).collect(), Vec::new())
    }

    #[tokio::test]
    async fn stream_yields_one_chunk_per_page() -> Result<(), Error> {
        let mut h = handler(&["page one", "page two", "page three"]);
        let mut pages = Vec::new();
        while let Some(chunk) = h.next_chunk().await? {
            pages.push(chunk.location.page_number);
        }
        assert_eq!(pages, vec![Some(1), Some(2), Some(3)]);
        Ok(())
    }

    #[tokio::test]
    async fn read_returns_page_text() -> Result<(), Error> {
        let mut h = handler(&["page one", "page two"]);
        let chunk = h.next_chunk().await?.unwrap();
        let data = h.read(&chunk.location).await?.unwrap();
        assert_eq!(data.as_str(), "page one");
        Ok(())
    }

    #[tokio::test]
    async fn read_cross_page_returns_none() -> Result<(), Error> {
        let h = handler(&["page one", "page two"]);
        let bogus = TextLocation {
            start: 5,
            end: 14,
            ..Default::default()
        };
        assert!(h.read(&bogus).await?.is_none());
        Ok(())
    }
}
