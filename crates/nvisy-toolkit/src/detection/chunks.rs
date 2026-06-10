//! Chunk-driven detection: walk an [`IndexedHandle<M>`]'s chunks,
//! run the [`RecognizerRegistry`] against each, and lift every
//! entity's offset back to source-coordinate `M::Location` via
//! [`IndexedHandle::lift_chunk`] before returning the merged list.
//!
//! Exposed as an extension trait
//! ([`RecognizerRegistryExt::detect`]) on [`RecognizerRegistry`]:
//! callers `use nvisy_toolkit::detection::RecognizerRegistryExt;`
//! and write `registry.detect(&mut handler).await?`. The handler's
//! modality determines the output entity type.
//!
//! Cell-shaped tabular values, JSON-escaped strings, and other
//! formats whose chunk payload doesn't match raw source bytes work
//! out of the box because the codec's `lift_chunk` knows how to
//! translate each one. The modality-to-modality reshape (turning
//! the recognizer's `Entity<Text>` into the handler's `Entity<M>`)
//! is handled by [`LiftedFromText`].
//!
//! Today recognizers are all `Text`-only — they scan a `&str` and
//! emit byte offsets — so detection always runs as `Text` against
//! the chunk payload and then reshapes upward. If/when a real
//! per-modality recognizer ships (e.g. image-only OCR-driven
//! recognition), it would skip this helper entirely and call
//! [`RecognizerRegistry::run`] for its own modality directly.
//!
//! [`IndexedHandle<M>`]: nvisy_codec::core::IndexedHandle
//! [`IndexedHandle::lift_chunk`]: nvisy_codec::core::IndexedHandle::lift_chunk

use nvisy_codec::core::{Codable, IndexedHandle};
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Modality, Tabular, TabularLocation, Text, TextData, TextLocation};
use nvisy_core::recognition::RecognizerInput;

use super::RecognizerRegistry;

/// Extension trait adding chunk-driven detection to
/// [`RecognizerRegistry`]. Each registered `Text` recognizer runs
/// against every chunk yielded by `handler`; matches are lifted to
/// source-coordinate `M::Location` via
/// [`IndexedHandle::lift_chunk`] and reshaped into `Entity<M>` via
/// [`LiftedFromText`].
///
/// Callers `use nvisy_toolkit::detection::RecognizerRegistryExt;`
/// to bring the method into scope.
///
/// [`IndexedHandle::lift_chunk`]: nvisy_codec::core::IndexedHandle::lift_chunk
#[async_trait::async_trait]
pub trait RecognizerRegistryExt {
    /// Walk `handler`'s chunks, run every registered `Text`
    /// recognizer against each chunk's data, lift each match to
    /// the handler's source-coordinate `M::Location`, and reshape
    /// into `Entity<M>`. Entities whose offsets fall outside the
    /// chunk's value (e.g. a match landing inside a JSON `\"`
    /// escape pair) are dropped: the lift has no source pre-image
    /// for them.
    async fn detect<M, H>(&self, handler: &mut H) -> Result<Vec<Entity<M>>>
    where
        M: Codable + LiftedFromText,
        M::Data: Into<TextData>,
        H: IndexedHandle<M> + ?Sized;
}

#[async_trait::async_trait]
impl RecognizerRegistryExt for RecognizerRegistry {
    async fn detect<M, H>(&self, handler: &mut H) -> Result<Vec<Entity<M>>>
    where
        M: Codable + LiftedFromText,
        M::Data: Into<TextData>,
        H: IndexedHandle<M> + ?Sized,
    {
        let mut out = Vec::new();
        while let Some(chunk) = handler.next_chunk().await? {
            let input = RecognizerInput::new(chunk.data.clone().into());
            let text_entities = self.run::<Text>(input).await?;
            for text_entity in text_entities {
                let Some(loc) = handler
                    .lift_chunk(&chunk, text_entity.location.start..text_entity.location.end)
                else {
                    continue;
                };
                out.push(M::from_text(text_entity, loc));
            }
        }
        Ok(out)
    }
}

/// Modality marker that knows how to absorb a `Text`-modality
/// finding into its own coordinate system.
///
/// Implementations describe the entity-level reshape only:
/// taking an `Entity<Text>` produced by a recognizer (with
/// chunk-local offsets) and a pre-lifted `Self::Location`,
/// produce an `Entity<Self>` with identity-relevant fields
/// preserved.
///
/// Today's impls:
///
/// - [`Text`] — identity: the recognizer's entity already carries
///   `Entity<Text>`; just install the lifted location.
/// - [`Tabular`] — rebuild with the lifted [`TabularLocation`];
///   preserve id, entity_id, kind, confidence, trail, language.
///
/// Future modalities (Audio, Image) would add impls here.
pub trait LiftedFromText: Modality + Sized {
    /// Take a [`Text`]-modality entity emitted by a recognizer
    /// against the source bytes of a chunk, plus the pre-lifted
    /// location, and produce a `Self`-modality entity.
    fn from_text(text_entity: Entity<Text>, location: Self::Location) -> Entity<Self>;
}

impl LiftedFromText for Text {
    fn from_text(mut entity: Entity<Text>, location: TextLocation) -> Entity<Text> {
        entity.location = location;
        entity
    }
}

impl LiftedFromText for Tabular {
    fn from_text(text_entity: Entity<Text>, location: TabularLocation) -> Entity<Tabular> {
        let mut builder = Entity::<Tabular>::builder()
            .with_id(text_entity.id)
            .with_entity_kind(text_entity.entity_kind)
            .with_location(location)
            .with_confidence(text_entity.confidence)
            .with_trail(text_entity.trail);
        if let Some(entity_id) = text_entity.entity_id {
            builder = builder.with_entity_id(entity_id);
        }
        if let Some(language) = text_entity.language {
            builder = builder.with_language(language);
        }
        builder.build().expect("entity reshape")
    }
}
