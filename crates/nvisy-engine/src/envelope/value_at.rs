//! [`ValueAt<M>`]: trait abstraction over [`DocumentEnvelope`]'s
//! per-modality `value_at` accessors.
//!
//! Each `DocumentEnvelope<M>` has its own concrete `value_at(&M)`
//! method (text reads from the codec handle, image consults the OCR
//! document, etc.). Generic engine code (deduplication, fusion)
//! parameterised over `M` calls into the right one via this trait.

use async_trait::async_trait;
use nvisy_ontology::modality::{Audio, Image, Modality, Tabular, Text};

use super::DocumentEnvelope;

/// Resolve a location of modality `M` to the corresponding source
/// text, for any envelope that knows how to look it up.
#[async_trait]
pub trait ValueAt<M: Modality>: Sync {
    /// Resolve a location to its source text representation, or
    /// `None` if the underlying handle / extraction document has
    /// nothing at that location.
    async fn value_at_loc(&self, location: &M) -> Option<String>;
}

#[async_trait]
impl ValueAt<Text> for DocumentEnvelope<Text> {
    async fn value_at_loc(&self, location: &Text) -> Option<String> {
        self.value_at(location).await
    }
}

#[async_trait]
impl ValueAt<Tabular> for DocumentEnvelope<Tabular> {
    async fn value_at_loc(&self, location: &Tabular) -> Option<String> {
        self.value_at(location).await
    }
}

#[async_trait]
impl ValueAt<Image> for DocumentEnvelope<Image> {
    async fn value_at_loc(&self, location: &Image) -> Option<String> {
        self.value_at(location).await
    }
}

#[async_trait]
impl ValueAt<Audio> for DocumentEnvelope<Audio> {
    async fn value_at_loc(&self, location: &Audio) -> Option<String> {
        self.value_at(location).await
    }
}
