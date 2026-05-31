//! [`ValueAt<M>`]: per-modality source-text lookup over a
//! [`DocumentEnvelope`].
//!
//! Each modality resolves a location differently: text/tabular read
//! the bytes from the codec handle, while image/audio walk the
//! extraction document's blocks for a matching region or span.
//! Generic engine code (deduplication, fusion, redaction) bounds
//! over `DocumentEnvelope<M>: ValueAt<M>` and calls into the right
//! implementation at the call site.

use nvisy_codec::handler::TextData;
use nvisy_ontology::modality::{Audio, AudioBlock, Image, ImageBlock, Modality, Tabular, Text};

use super::DocumentEnvelope;

/// Resolve a location of modality `M` to the corresponding source
/// text, for any envelope that knows how to look it up.
#[async_trait::async_trait]
pub trait ValueAt<M: Modality>: Sync {
    /// Resolve a location to its source text representation, or
    /// `None` if the underlying handle / extraction document has
    /// nothing at that location.
    async fn value_at(&self, location: &M) -> Option<String>;
}

#[async_trait::async_trait]
impl ValueAt<Text> for DocumentEnvelope<Text> {
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
impl ValueAt<Tabular> for DocumentEnvelope<Tabular> {
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
impl ValueAt<Image> for DocumentEnvelope<Image> {
    /// Exact bounding-box match against a block's `region` returns
    /// the whole block text; sub-region matches consult the block's
    /// `spans`.
    async fn value_at(&self, location: &Image) -> Option<String> {
        for block in &self.document.blocks {
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
impl ValueAt<Audio> for DocumentEnvelope<Audio> {
    /// Exact time-span match against a `Speech` block returns the
    /// whole transcript; sub-segment matches consult the block's
    /// `spans`.
    async fn value_at(&self, location: &Audio) -> Option<String> {
        for block in &self.document.blocks {
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
