//! [`TextAt<M>`], [`DataAt<M>`], and [`RedactAt<M>`] impls on
//! [`DocumentTree<M>`].
//!
//! Phases that need to resolve a modality-typed location to its source
//! text or wire payload take `&DocumentTree<M>` directly — there is no
//! separate view type. Text and tabular reads go through the codec
//! handle; image and audio reads walk the document's blocks. Write-back
//! ([`RedactAt::redact_at`]) always goes through the codec handle.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::extraction::{DataAt, TextAt};
use nvisy_core::modality::{
    Audio, AudioData, AudioLocation, Image, ImageData, ImageLocation, Tabular, TabularLocation,
    Text, TextData, TextLocation,
};
use nvisy_core::redaction::{RedactAt, Redactions};

use super::DocumentTree;
use crate::modality::{AudioBlock, ImageBlock};

#[async_trait]
impl TextAt<Text> for DocumentTree<Text> {
    /// Resolve a [`Text`] location to its source text via the codec
    /// handle. Returns `None` when the handle has no readable bytes
    /// at the location.
    async fn text_at(&self, location: &TextLocation) -> Option<String> {
        self.handle
            .handler()
            .read(location)
            .await
            .ok()
            .flatten()
            .map(|d| d.into_string())
    }
}

#[async_trait]
impl TextAt<Tabular> for DocumentTree<Tabular> {
    /// Resolve a [`Tabular`] location to its source cell value via the
    /// codec handle.
    async fn text_at(&self, location: &TabularLocation) -> Option<String> {
        self.handle
            .handler()
            .read(location)
            .await
            .ok()
            .flatten()
            .map(|d| d.into_string())
    }
}

#[async_trait]
impl TextAt<Image> for DocumentTree<Image> {
    /// Resolve an [`Image`] location to the OCR'd text at that region
    /// by walking the document's blocks. Exact bounding-box match
    /// against a block's `region` returns the whole block text;
    /// sub-region matches consult the block's `spans`.
    async fn text_at(&self, location: &ImageLocation) -> Option<String> {
        for block in &self.root.blocks {
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

#[async_trait]
impl TextAt<Audio> for DocumentTree<Audio> {
    /// Resolve an [`Audio`] location to the transcript at that time
    /// span by walking the document's blocks.
    async fn text_at(&self, location: &AudioLocation) -> Option<String> {
        for block in &self.root.blocks {
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

#[async_trait]
impl DataAt<Text> for DocumentTree<Text> {
    /// Resolve a [`Text`] location to its [`TextData`] payload via
    /// the codec handle.
    async fn data_at(&self, location: &TextLocation) -> Option<TextData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[async_trait]
impl DataAt<Image> for DocumentTree<Image> {
    /// Resolve an [`Image`] location to its [`ImageData`] payload via
    /// the codec handle.
    async fn data_at(&self, location: &ImageLocation) -> Option<ImageData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[async_trait]
impl DataAt<Audio> for DocumentTree<Audio> {
    /// Resolve an [`Audio`] location to its [`AudioData`] payload via
    /// the codec handle.
    async fn data_at(&self, location: &AudioLocation) -> Option<AudioData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[async_trait]
impl RedactAt<Text> for DocumentTree<Text> {
    /// Apply a batch of text replacements through the codec handle.
    /// The handler decides per-format ordering (right-to-left for byte
    /// streams, batched per page for PDF, …).
    async fn redact_at(&mut self, redactions: Redactions<Text>) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}

#[async_trait]
impl RedactAt<Image> for DocumentTree<Image> {
    /// Apply a batch of image replacements through the codec handle.
    async fn redact_at(&mut self, redactions: Redactions<Image>) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}

#[async_trait]
impl RedactAt<Audio> for DocumentTree<Audio> {
    /// Apply a batch of audio replacements through the codec handle.
    async fn redact_at(&mut self, redactions: Redactions<Audio>) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}
