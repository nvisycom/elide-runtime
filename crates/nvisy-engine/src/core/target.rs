//! [`SharedHandle`] + [`DocumentView`] + [`ValueAt`]: the read-only
//! per-doc surface phase bodies use to resolve a modality-typed
//! location to its source string.

use std::sync::Arc;

use nvisy_codec::DocumentHandle;
use nvisy_codec::handler::TextData;
use nvisy_ontology::document::Document;
use nvisy_ontology::modality::{Audio, AudioBlock, Image, ImageBlock, Modality, Tabular, Text};
use tokio::sync::Mutex;

/// Shared codec handle phases borrow for source reads and redaction
/// writes. Wrapped in `Arc<Mutex<_>>` because handle redaction
/// methods take `&mut self`; phases coordinate access to the
/// underlying document through the lock.
///
/// Constructed by the orchestrator from the codec output of the
/// ingestion phase; phases see it as part of every
/// [`DocumentTree`].
///
/// [`DocumentTree`]: super::DocumentTree
pub type SharedHandle = Arc<Mutex<DocumentHandle>>;

/// Resolve a location of modality `M` to the corresponding source
/// text, for any per-call surface that knows how to look it up.
///
/// Implemented by [`DocumentView<'_, M>`] (a read-only doc+handle pair).
/// Generic phase code (dedup, redaction, validation) bounds over
/// `&impl ValueAt<M>` and dispatches uniformly.
#[async_trait::async_trait]
pub trait ValueAt<M: Modality>: Sync {
    /// Resolve a location to its source text representation, or
    /// `None` if the underlying handle / extraction document has
    /// nothing at that location.
    async fn value_at(&self, location: &M) -> Option<String>;
}

/// Read-only view over `(doc, handle)` carrying the per-modality
/// [`ValueAt`] impls. Phase bodies that need to resolve source
/// text at a typed location take a `&DocumentView<'_, M>` constructed
/// once at the top of their dispatch.
pub struct DocumentView<'a, M: Modality> {
    /// The document the value resolver reads from. For image/audio
    /// modalities this is the source of the recognised text (no
    /// handle lookup); for text/tabular the handle is consulted.
    pub doc: &'a Document<M>,
    /// Shared codec handle for source reads (text/tabular). Held
    /// even when unused so callers can construct the view uniformly.
    pub handle: &'a SharedHandle,
}

impl<'a, M: Modality> DocumentView<'a, M> {
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
    async fn value_at(&self, location: &Text) -> Option<String> {
        self.handle
            .lock()
            .await
            .read_text(location)
            .await
            .map(TextData::into_inner)
    }
}

#[async_trait::async_trait]
impl ValueAt<Tabular> for DocumentView<'_, Tabular> {
    /// Resolve a [`Tabular`] location to its source cell value via
    /// the codec handle.
    async fn value_at(&self, location: &Tabular) -> Option<String> {
        self.handle
            .lock()
            .await
            .read_tabular(location)
            .await
            .map(TextData::into_inner)
    }
}

#[async_trait::async_trait]
impl ValueAt<Image> for DocumentView<'_, Image> {
    /// Resolve an [`Image`] location to the OCR'd text at that
    /// region by walking the document's blocks. Exact bounding-box
    /// match against a block's `region` returns the whole block
    /// text; sub-region matches consult the block's `spans`.
    ///
    /// Doc-only (no handle access): image source bytes don't have a
    /// "value at this region" notion — the value is the recognised
    /// text the extraction phase already stored.
    async fn value_at(&self, location: &Image) -> Option<String> {
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
    async fn value_at(&self, location: &Audio) -> Option<String> {
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
