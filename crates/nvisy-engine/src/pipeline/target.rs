//! [`PhaseTarget`]: the narrow per-call surface every [`Phase`]
//! sees.
//!
//! Phases used to take `&mut DocumentEnvelope<M>` directly, which made
//! the envelope a god-object (see `.ignore/engine-architecture.md`
//! Q2): every phase could in principle reach every envelope field,
//! even ones it had no business touching. `PhaseTarget` bundles only
//! what phases actually need — a mutable [`Document<M>`] and a
//! [`SharedHandle`], plus the per-run [`run_id`] and the
//! [`ContentMetadata`] that policy evaluation reads — so the
//! envelope can stay an orchestrator-only carrier.
//!
//! The same shape also enables Q19's nested-document recursion: the
//! orchestrator builds a fresh `PhaseTarget` for each
//! [`TextBlock::Embed`] child it walks into, pointing at the nested
//! [`Document<M>`] but borrowing the outer envelope's handle (per
//! the locked decision that nested docs are data-only and don't own
//! a codec handle).
//!
//! [`Phase`]: super::Phase
//! [`Document<M>`]: nvisy_ontology::document::Document
//! [`SharedHandle`]: crate::envelope::SharedHandle
//! [`ContentMetadata`]: nvisy_core::content::ContentMetadata
//! [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
//! [`run_id`]: Self::run_id

use std::sync::Arc;

use nvisy_codec::handler::TextData;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::document::Document;
use nvisy_ontology::modality::{Audio, AudioBlock, Image, ImageBlock, Modality, Tabular, Text};
use uuid::Uuid;

use crate::envelope::value_at::ValueAt;
use crate::envelope::{SharedData, SharedHandle};

/// Narrow per-call view every [`Phase`] runs against.
///
/// Holds the mutable [`Document<M>`] the phase reads and writes, the
/// shared codec [`SharedHandle`] for source reads / redaction
/// writes, and the per-run id + content metadata phases need for
/// correlation and policy matching. Phases never see the envelope
/// itself.
///
/// [`Phase`]: super::Phase
pub struct PhaseTarget<'a, M: Modality> {
    /// The document this phase operates on. For the root pass this
    /// is the envelope's document; for nested-doc recursion (a
    /// `Document<Image>` or `Document<Tabular>` inside a
    /// [`TextBlock::Embed`]) this is the nested doc, not the outer.
    ///
    /// [`TextBlock::Embed`]: nvisy_ontology::modality::TextBlock::Embed
    pub doc: &'a mut Document<M>,
    /// Shared codec handle for source reads and redaction writes.
    /// Always the *outer* envelope's handle, even when `doc` is a
    /// nested document — nested docs are data-only.
    pub handle: &'a SharedHandle,
    /// Per-run correlation id. Phases stamp this into recognizer
    /// contexts so external services can correlate requests back to
    /// the engine run.
    pub run_id: Uuid,
    /// Content metadata (MIME type, filename, …) from the original
    /// upload. Policy evaluation matches rules against fields here;
    /// detection forwards it to recognizers that need source context.
    pub metadata: &'a ContentMetadata,
    /// Run-wide shared state — policies, registry, key provider.
    /// Read-only from the phase's perspective; mutation happens via
    /// other channels (registry persistence, policy reloads at run
    /// boundaries).
    pub shared: &'a Arc<SharedData>,
}

impl<'a, M: Modality> PhaseTarget<'a, M> {
    /// Build a `PhaseTarget` from its borrowed parts. Factored out so
    /// the orchestrator (root pass) and the nested-doc tree walker
    /// share one construction path.
    pub(crate) fn new(
        doc: &'a mut Document<M>,
        handle: &'a SharedHandle,
        run_id: Uuid,
        metadata: &'a ContentMetadata,
        shared: &'a Arc<SharedData>,
    ) -> Self {
        Self {
            doc,
            handle,
            run_id,
            metadata,
            shared,
        }
    }
}

#[async_trait::async_trait]
impl ValueAt<Text> for PhaseTarget<'_, Text> {
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
impl ValueAt<Tabular> for PhaseTarget<'_, Tabular> {
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
impl ValueAt<Image> for PhaseTarget<'_, Image> {
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
impl ValueAt<Audio> for PhaseTarget<'_, Audio> {
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
