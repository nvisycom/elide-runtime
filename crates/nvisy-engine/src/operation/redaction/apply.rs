//! Compute replacement values and apply redaction records to
//! document content via codec transforms.
//!
//! [`RedactionApplicator`] holds a mutable reference to the envelope,
//! iterates `audit.entries`, builds per-modality codec instructions,
//! writes `replaced_value` inline, and applies instructions to the
//! document.

use std::collections::HashMap;

use nvisy_codec::transform::{
    AudioOutput, AudioRedaction, ImageOutput, ImageRedaction, TextOutput, TextRedaction,
};
use nvisy_ontology::entity::{
    AudioLocation, Entity, EntityKind, ImageLocation, Location, TextLocation,
};
use nvisy_ontology::policy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::operation::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::op::redaction::apply";

const IMAGE_REDACTED: &str = "[IMAGE_REDACTED]";
const AUDIO_REDACTED: &str = "[AUDIO_REDACTED]";

/// Builds and applies redaction instructions across all modalities.
///
/// Holds `&mut DocumentEnvelope` and iterates `audit.entries` to build
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

    fn build_text_redactions(&mut self) -> Vec<TextRedaction<TextLocation>> {
        let entity_map = Self::entity_map(&self.envelope.audit.entities);
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let entity = match entity_map.get(&record.entity_id) {
                Some(e) => *e,
                None => continue,
            };

            let Location::Text(ref loc) = entity.location else {
                continue;
            };

            let output = match &record.redaction.strategy {
                Strategy::Text(text) => Self::text_output(entity, text),
                _ => continue,
            };

            let entity_id = record.entity_id;

            let replacement = output.replacement_value().map(String::from);
            self.envelope.audit.entries[i].value.replacement = replacement.clone();
            if let Some(mapping) = self
                .envelope
                .redaction_map
                .entries
                .iter_mut()
                .find(|m| m.entity_id == entity_id)
            {
                mapping.replacement = replacement;
            }

            tracing::trace!(
                target: TARGET,
                %entity_id,
                start = loc.start_offset,
                end = loc.end_offset,
                "built text redaction instruction",
            );

            // The entity location directly identifies the byte range
            // to redact. start/end are intra-span offsets (0..len for
            // a full value replacement within the containing span).
            redactions.push(TextRedaction {
                span_id: loc.clone(),
                start: loc.start_offset,
                end: loc.end_offset,
                output,
            });
        }

        redactions
    }

    fn build_image_redactions(&mut self) -> Vec<ImageRedaction<ImageLocation>> {
        let entity_map = Self::entity_map(&self.envelope.audit.entities);
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let entity = match entity_map.get(&record.entity_id) {
                Some(e) => *e,
                None => continue,
            };

            let Location::Image(ref loc) = entity.location else {
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

            let entity_id = record.entity_id;
            let replacement = IMAGE_REDACTED.to_string();

            self.envelope.audit.entries[i].value.replacement = Some(replacement.clone());
            if let Some(mapping) = self
                .envelope
                .redaction_map
                .entries
                .iter_mut()
                .find(|m| m.entity_id == entity_id)
            {
                mapping.replacement = Some(replacement);
            }

            tracing::trace!(
                target: TARGET,
                %entity_id,
                "built image redaction instruction",
            );

            redactions.push(ImageRedaction {
                span_id: loc.clone(),
                bounding_box: loc.bounding_box,
                output,
            });
        }

        redactions
    }

    fn build_audio_redactions(&mut self) -> Vec<AudioRedaction<AudioLocation>> {
        let entity_map = Self::entity_map(&self.envelope.audit.entities);
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let entity = match entity_map.get(&record.entity_id) {
                Some(e) => *e,
                None => continue,
            };

            let Location::Audio(ref loc) = entity.location else {
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
            let replacement = AUDIO_REDACTED.to_string();

            self.envelope.audit.entries[i].value.replacement = Some(replacement.clone());
            if let Some(mapping) = self
                .envelope
                .redaction_map
                .entries
                .iter_mut()
                .find(|m| m.entity_id == entity_id)
            {
                mapping.replacement = Some(replacement);
            }

            tracing::trace!(
                target: TARGET,
                %entity_id,
                start_us = loc.time_span.start_us,
                end_us = loc.time_span.end_us,
                "built audio redaction instruction",
            );

            redactions.push(AudioRedaction {
                span_id: loc.clone(),
                time_span: loc.time_span,
                output,
            });
        }

        redactions
    }

    /// Build a lookup map from entity UUID to entity reference.
    fn entity_map(entities: &nvisy_ontology::entity::Entities) -> HashMap<Uuid, &Entity> {
        entities.iter().map(|e| (e.id, e)).collect()
    }

    fn text_output(entity: &Entity, strategy: &TextStrategy) -> TextOutput {
        let value = entity.text_value().unwrap_or_default();
        match strategy {
            TextStrategy::Mask { mask_char } => {
                TextOutput::replace(mask_char.to_string().repeat(value.len()))
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

            TextStrategy::Hash => TextOutput::replace(Self::hash_value(value)),

            TextStrategy::Pseudonymize => {
                TextOutput::replace(Self::pseudonymize(&entity.entity_kind, value))
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
    use nvisy_ontology::provenance::AuditEntry;

    use super::*;
    use crate::operation::envelope::SharedData;

    fn text_entity(value: &str, start: usize, end: usize) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_text(value)
                    .with_start_offset(start)
                    .with_end_offset(end)
                    .build()
                    .unwrap(),
            ))
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

    fn test_record(entity_id: Uuid, strategy: Strategy, original: &str) -> AuditEntry {
        AuditEntry::builder()
            .for_entity(entity_id, strategy, original)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn mask_applies_and_records_replacement() {
        let entity = text_entity("John", 6, 10);
        let entity_id = entity.id;
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            "John",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert_eq!(
            envelope.audit.entries[0].value.replacement.as_deref(),
            Some("****"),
        );
    }

    #[tokio::test]
    async fn remove_leaves_replacement_none() {
        let entity = text_entity("John", 6, 10);
        let entity_id = entity.id;
        let record = test_record(entity_id, Strategy::Text(TextStrategy::Remove), "John");

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert!(envelope.audit.entries[0].value.replacement.is_none());
    }

    #[tokio::test]
    async fn skips_image_strategy_for_text_entity() {
        let entity = text_entity("face", 0, 4);
        let record = test_record(
            entity.id,
            Strategy::Image(ImageStrategy::Blur { sigma: 15.0 }),
            "face",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

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
