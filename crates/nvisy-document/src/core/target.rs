//! [`SharedHandle`] + [`DocumentView`]: the read-only per-doc
//! surface phase bodies use to resolve a modality-typed location.
//! Two flavours, both keyed by the same `M::Location`:
//!
//! - [`ValueAt`] — text-shaped (`String`) reads consumed by dedup
//!   layers and validation checks.
//! - [`SourceAt`] — full `M::Data` reads consumed by redaction
//!   operators.
//!
//! Both traits live in `nvisy-core`; this module supplies the
//! document-side impls.

use std::sync::Arc;

use nvisy_codec::DocumentHandle;
use nvisy_codec::handler::TextData as CodecTextData;
use nvisy_core::ValueAt;
use nvisy_core::extraction::SourceAt;
use nvisy_core::modality::{
    Audio, AudioData, AudioLocation, Image, ImageData, ImageLocation, Tabular, TabularLocation,
    Text, TextData, TextLocation,
};
use tokio::sync::Mutex;

use crate::document::Document;
use crate::modality::{AudioBlock, DocumentModality, ImageBlock};

/// Shared codec handle phases borrow for source reads and redaction
/// writes. Wrapped in `Arc<Mutex<_>>` because handle redaction
/// methods take `&mut self`; phases coordinate access to the
/// underlying document through the lock.
///
/// Constructed by the orchestrator from the codec output of the
/// ingestion phase; phases see it as part of every [`DocumentTree`].
///
/// [`DocumentTree`]: super::DocumentTree
pub type SharedHandle = Arc<Mutex<DocumentHandle>>;

/// Read-only view over `(doc, handle)` carrying the per-modality
/// [`ValueAt`] impls. Phase bodies that need to resolve source
/// text at a typed location take a `&DocumentView<'_, M>` constructed
/// once at the top of their dispatch.
pub struct DocumentView<'a, M: DocumentModality> {
    /// The document the value resolver reads from. For image/audio
    /// modalities this is the source of the recognised text (no
    /// handle lookup); for text/tabular the handle is consulted.
    pub doc: &'a Document<M>,
    /// Shared codec handle for source reads (text/tabular). Held
    /// even when unused so callers can construct the view uniformly.
    pub handle: &'a SharedHandle,
}

impl<'a, M: DocumentModality> DocumentView<'a, M> {
    /// Construct a doc+handle view. Borrow-only — does not take
    /// ownership and does not lock the handle.
    pub fn new(doc: &'a Document<M>, handle: &'a SharedHandle) -> Self {
        Self { doc, handle }
    }
}

#[async_trait::async_trait]
impl ValueAt<Text> for DocumentView<'_, Text> {
    /// Resolve a [`Text`] location to its source text via the codec
    /// handle. Returns `None` when the handle has no readable bytes
    /// at the location.
    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        self.handle
            .lock()
            .await
            .read_text(location)
            .await
            .map(CodecTextData::into_inner)
    }
}

#[async_trait::async_trait]
impl ValueAt<Tabular> for DocumentView<'_, Tabular> {
    /// Resolve a [`Tabular`] location to its source cell value via
    /// the codec handle.
    async fn value_at(&self, location: &TabularLocation) -> Option<String> {
        self.handle
            .lock()
            .await
            .read_tabular(location)
            .await
            .map(CodecTextData::into_inner)
    }
}

#[async_trait::async_trait]
impl ValueAt<Image> for DocumentView<'_, Image> {
    /// Resolve an [`Image`] location to the OCR'd text at that
    /// region by walking the document's blocks. Exact bounding-box
    /// match against a block's `region` returns the whole block
    /// text; sub-region matches consult the block's `spans`.
    async fn value_at(&self, location: &ImageLocation) -> Option<String> {
        for block in &self.doc.blocks {
            let (text, region) = match &block.kind {
                ImageBlock::Text { text, region }
                | ImageBlock::Heading { text, region }
                | ImageBlock::Table { text, region } => (text, region),
                _ => continue,
            };
            if region == location {
                return Some(text.clone());
            }
            if let Some(s) = block.spans.iter().find(|s| s.source == *location) {
                return Some(text[s.text_start..s.text_end].to_owned());
            }
        }
        None
    }
}

#[async_trait::async_trait]
impl ValueAt<Audio> for DocumentView<'_, Audio> {
    /// Resolve an [`Audio`] location to the transcript at that time
    /// span by walking the document's blocks. Exact match returns
    /// the whole `Speech` block; sub-segment matches consult spans.
    async fn value_at(&self, location: &AudioLocation) -> Option<String> {
        for block in &self.doc.blocks {
            let AudioBlock::Speech {
                text, time_span, ..
            } = &block.kind
            else {
                continue;
            };
            if time_span == &location.time_span {
                return Some(text.clone());
            }
            if let Some(s) = block.spans.iter().find(|s| s.source == *location) {
                return Some(text[s.text_start..s.text_end].to_owned());
            }
        }
        None
    }
}

#[async_trait::async_trait]
impl SourceAt<Text> for DocumentView<'_, Text> {
    /// Resolve a [`Text`] location to its core-shaped [`TextData`]
    /// payload via the codec handle. The codec returns a
    /// `nvisy_codec::handler::TextData`; this adapter re-wraps the
    /// inner string into the richer `nvisy_core::TextData` shape that
    /// recognizers and anonymizers consume.
    async fn source_at(&self, location: &TextLocation) -> Option<TextData> {
        let raw = self.handle.lock().await.read_text(location).await?;
        Some(TextData::new(raw.into_inner()))
    }
}

#[async_trait::async_trait]
impl SourceAt<Image> for DocumentView<'_, Image> {
    /// Resolve an [`Image`] location to its core-shaped [`ImageData`]
    /// payload via the codec handle. The codec hands back a decoded
    /// `DynamicImage` wrapper; this adapter re-encodes to PNG bytes
    /// + records the dimensions to match the
    /// `(bytes, dims)` shape recognizers and anonymizers operate on.
    async fn source_at(&self, location: &ImageLocation) -> Option<ImageData> {
        let raw = self.handle.lock().await.read_image(location).await?;
        let dims = raw.dimensions();
        let bytes = raw.encode_png().ok()?;
        Some(ImageData::new(bytes, dims))
    }
}

#[async_trait::async_trait]
impl SourceAt<Audio> for DocumentView<'_, Audio> {
    /// Resolve an [`Audio`] location to its core-shaped [`AudioData`]
    /// payload via the codec handle.
    ///
    /// The codec wraps raw bytes; the core shape carries a filename
    /// alongside. The original filename isn't tracked at the
    /// per-location level, so the wrapper here stamps an empty
    /// filename — anonymizers that don't read the filename are
    /// unaffected.
    async fn source_at(&self, location: &AudioLocation) -> Option<AudioData> {
        let raw = self.handle.lock().await.read_audio(location).await?;
        Some(AudioData::new(raw.as_bytes().clone(), ""))
    }
}
