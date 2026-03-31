//! Compute replacement values and apply redaction records to
//! document content via codec transforms.
//!
//! [`RedactionApplicator`] holds a mutable reference to the envelope,
//! iterates `audit.records`, builds per-modality codec instructions,
//! writes `replaced_value` inline, and applies instructions to the
//! document.

use std::collections::HashMap;

use nvisy_codec::handler::{AudioSpanId, ImageSpanId, TextSpanId};
use nvisy_codec::transform::{
    AudioOutput, AudioRedaction, ImageOutput, ImageRedaction, TextOutput, TextRedaction,
};
use nvisy_ontology::entity::{Entity, EntityKind, Location};
use nvisy_ontology::policy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::operation::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::op::redaction::apply";

const IMAGE_REDACTED: &str = "[IMAGE_REDACTED]";
const AUDIO_REDACTED: &str = "[AUDIO_REDACTED]";

/// Builds and applies redaction instructions across all modalities.
///
/// Holds `&mut DocumentEnvelope` and iterates `audit.records` to build
/// codec instructions. Writes `replaced_value` directly into each
/// record during the build phase.
pub(super) struct RedactionApplicator<'a> {
    envelope: &'a mut DocumentEnvelope,
}

impl<'a> RedactionApplicator<'a> {
    pub fn new(envelope: &'a mut DocumentEnvelope) -> Self {
        Self { envelope }
    }

    /// Build and apply all redaction instructions.
    pub async fn apply(mut self) -> nvisy_core::Result<()> {
        let text = self.build_text_redactions();
        let image = self.build_image_redactions();
        let audio = self.build_audio_redactions();

        if !text.is_empty() {
            self.envelope.document.apply_text_redactions(&text).await?;
        }
        if !image.is_empty() {
            self.envelope
                .document
                .apply_image_redactions(&image)
                .await?;
        }
        if !audio.is_empty() {
            self.envelope
                .document
                .apply_audio_redactions(&audio)
                .await?;
        }

        Ok(())
    }

    fn build_text_redactions(&mut self) -> Vec<TextRedaction<TextSpanId>> {
        let entity_map = Self::entity_map(&self.envelope.audit.entities);
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.records.len() {
            let record = &self.envelope.audit.records[i];
            let entity = match entity_map.get(&record.entity_id) {
                Some(e) => *e,
                None => continue,
            };

            let Some(Location::Text(ref loc)) = entity.location else {
                continue;
            };

            let output = match &record.redaction.strategy {
                Strategy::Text(text) => Self::text_output(entity, text),
                _ => continue,
            };

            let Some(index) = loc.span_index else {
                continue;
            };
            let span_id = TextSpanId(index);
            let entity_id = record.entity_id;

            self.envelope.audit.records[i].value.replacement =
                output.replacement_value().map(String::from);

            tracing::trace!(
                target: TARGET,
                %entity_id,
                span = span_id.0,
                start = loc.start_offset,
                end = loc.end_offset,
                "built text redaction instruction",
            );

            redactions.push(TextRedaction {
                span_id,
                start: loc.start_offset,
                end: loc.end_offset,
                output,
            });
        }

        redactions
    }

    fn build_image_redactions(&mut self) -> Vec<ImageRedaction<ImageSpanId>> {
        let entity_map = Self::entity_map(&self.envelope.audit.entities);
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.records.len() {
            let record = &self.envelope.audit.records[i];
            let entity = match entity_map.get(&record.entity_id) {
                Some(e) => *e,
                None => continue,
            };

            let Some(Location::Image(ref loc)) = entity.location else {
                continue;
            };

            let output = match &record.redaction.strategy {
                Strategy::Image(img) => match img {
                    ImageStrategy::Blur { sigma } => ImageOutput::Blur { sigma: *sigma },
                    ImageStrategy::Block { color } => ImageOutput::Block { color: *color },
                    ImageStrategy::Pixelate { block_size } => ImageOutput::Pixelate {
                        block_size: *block_size,
                    },
                    _ => continue,
                },
                _ => continue,
            };

            let span_id = ImageSpanId(loc.page_number);
            let entity_id = record.entity_id;

            self.envelope.audit.records[i].value.replacement = Some(IMAGE_REDACTED.into());

            tracing::trace!(
                target: TARGET,
                %entity_id,
                "built image redaction instruction",
            );

            redactions.push(ImageRedaction {
                span_id,
                bounding_box: loc.bounding_box,
                output,
            });
        }

        redactions
    }

    fn build_audio_redactions(&mut self) -> Vec<AudioRedaction<AudioSpanId>> {
        let entity_map = Self::entity_map(&self.envelope.audit.entities);
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.records.len() {
            let record = &self.envelope.audit.records[i];
            let entity = match entity_map.get(&record.entity_id) {
                Some(e) => *e,
                None => continue,
            };

            let Some(Location::Audio(ref loc)) = entity.location else {
                continue;
            };

            let output = match &record.redaction.strategy {
                Strategy::Audio(audio) => match audio {
                    AudioStrategy::Silence => AudioOutput::Silence,
                    AudioStrategy::Remove => AudioOutput::Remove,
                    _ => continue,
                },
                _ => continue,
            };

            let entity_id = record.entity_id;

            self.envelope.audit.records[i].value.replacement = Some(AUDIO_REDACTED.into());

            tracing::trace!(
                target: TARGET,
                %entity_id,
                start = loc.time_span.start_secs,
                end = loc.time_span.end_secs,
                "built audio redaction instruction",
            );

            redactions.push(AudioRedaction {
                span_id: AudioSpanId::default(),
                time_span: loc.time_span,
                output,
            });
        }

        redactions
    }

    /// Build a lookup map from entity UUID to entity reference.
    fn entity_map(entities: &nvisy_ontology::entity::Entities) -> HashMap<Uuid, &Entity> {
        entities.iter().map(|e| (e.source.as_uuid(), e)).collect()
    }

    fn text_output(entity: &Entity, strategy: &TextStrategy) -> TextOutput {
        match strategy {
            TextStrategy::Mask { mask_char } => {
                TextOutput::replace(mask_char.to_string().repeat(entity.value.len()))
            }

            TextStrategy::Replace { placeholder } => {
                if placeholder.is_empty() {
                    TextOutput::replace(format!(
                        "[{}]",
                        entity.entity_kind.to_string().to_uppercase()
                    ))
                } else {
                    TextOutput::replace(
                        placeholder
                            .replace("{entityType}", &entity.entity_kind.to_string())
                            .replace("{category}", &entity.category.to_string()),
                    )
                }
            }

            TextStrategy::Remove => TextOutput::Remove,

            TextStrategy::Hash => TextOutput::replace(Self::hash_value(&entity.value)),

            TextStrategy::Pseudonymize => {
                TextOutput::replace(Self::pseudonymize(&entity.entity_kind, &entity.value))
            }

            TextStrategy::Encrypt { .. } => {
                TextOutput::replace(format!("[ENC:{}]", entity.entity_kind))
            }
            TextStrategy::Tokenize { .. } => {
                TextOutput::replace(format!("[TOKEN:{}]", entity.entity_kind))
            }
            _ => TextOutput::replace(format!("[REDACTED:{}]", entity.entity_kind)),
        }
    }

    fn hash_value(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        let hash = hasher.finalize();
        hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }

    fn pseudonymize(entity_kind: &EntityKind, value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(entity_kind.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(value.as_bytes());
        let hash = hasher.finalize();
        let id: u32 = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
        format!("{entity_kind}_{id}")
    }
}

#[cfg(test)]
mod tests {
    use nvisy_codec::Document;
    use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
    use nvisy_ontology::entity::{
        Entities, Entity, EntityCategory, EntityKind, Location, RecognitionMethod, TextLocation,
    };
    use nvisy_ontology::policy::ImageStrategy;
    use nvisy_ontology::provenance::RedactionRecord;

    use super::*;
    use crate::operation::envelope::SharedData;

    fn text_entity(value: &str, span_index: usize, start: usize, end: usize) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_value(value)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .with_location(Location::from(TextLocation {
                start_offset: start,
                end_offset: end,
                span_index: Some(span_index),
                ..Default::default()
            }))
            .build()
            .unwrap()
    }

    async fn test_envelope(entities: Entities) -> DocumentEnvelope {
        let data = ContentData::from_text(ContentSource::new(), "Hello John world");
        let content =
            Content::with_metadata(data, ContentMetadata::new().with_content_type("text/plain"));
        let doc = Document::decode(&content).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::registry::Registry::open(dir.path()).unwrap();
        let shared = SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        let mut envelope = DocumentEnvelope::new(doc, ContentMetadata::default(), shared);
        envelope.audit.entities = entities;
        envelope
    }

    fn test_record(entity_id: Uuid, strategy: Strategy, original: &str) -> RedactionRecord {
        RedactionRecord::builder()
            .for_entity(entity_id, strategy, original, 0.9)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn mask_applies_and_records_replacement() {
        let entity = text_entity("John", 0, 6, 10);
        let entity_id = entity.source.as_uuid();
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            "John",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.records.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert_eq!(
            envelope.audit.records[0].value.replacement.as_deref(),
            Some("****"),
        );
    }

    #[tokio::test]
    async fn remove_leaves_replacement_none() {
        let entity = text_entity("John", 0, 6, 10);
        let entity_id = entity.source.as_uuid();
        let record = test_record(entity_id, Strategy::Text(TextStrategy::Remove), "John");

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.records.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert!(envelope.audit.records[0].value.replacement.is_none());
    }

    #[tokio::test]
    async fn skips_image_strategy_for_text_entity() {
        let entity = text_entity("face", 0, 0, 4);
        let record = test_record(
            entity.source.as_uuid(),
            Strategy::Image(ImageStrategy::Blur { sigma: 15.0 }),
            "face",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.records.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();
    }

    #[test]
    fn hash_replacement_is_deterministic() {
        let a = RedactionApplicator::hash_value("John Smith");
        let b = RedactionApplicator::hash_value("John Smith");
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn pseudonymize_is_deterministic() {
        let a = RedactionApplicator::pseudonymize(&EntityKind::PersonName, "John Smith");
        let b = RedactionApplicator::pseudonymize(&EntityKind::PersonName, "John Smith");
        assert_eq!(a, b);
        assert!(a.contains('_'));
    }
}
