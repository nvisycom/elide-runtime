//! Compute replacement values and apply redaction decisions to
//! document content via codec transforms.
//!
//! [`RedactionApplicator`] holds a mutable reference to the envelope
//! and reads decisions/entities, builds per-modality codec instructions,
//! writes `replaced_value` into audit records, and applies instructions
//! to the document.

use nvisy_codec::handler::{AudioSpanId, ImageSpanId, TextSpanId};
use nvisy_codec::transform::{
    AudioOutput, AudioRedaction, ImageOutput, ImageRedaction, TextOutput, TextRedaction,
};
use nvisy_ontology::entity::{Entity, EntityKind, Location};
use nvisy_ontology::policy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};
use nvisy_ontology::provenance::{RedactionDecision, RedactionRecord};
use sha2::{Digest, Sha256};

use crate::operation::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::op::redaction::apply";

/// Builds and applies redaction instructions across all modalities.
///
/// Holds `&mut DocumentEnvelope` and accesses its fields directly:
/// reads `audit.decisions` and `entities`, writes `audit.records`,
/// and mutates `document`.
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
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.decisions.len() {
            let decision = &self.envelope.audit.decisions[i];
            let entity = match Self::find_entity(&self.envelope.entities, decision) {
                Some(e) => e,
                None => continue,
            };

            let Some(Location::Text(ref loc)) = entity.location else {
                continue;
            };

            let output = match &decision.spec {
                Strategy::Text(text) => Self::text_output(entity, text),
                _ => continue,
            };

            let Some(index) = loc.span_index else {
                continue;
            };
            let span_id = TextSpanId(index);
            let entity_id = decision.entity_id;

            Self::set_replaced_value(
                &mut self.envelope.audit.records,
                entity_id,
                output.replacement_value().map(String::from),
            );

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
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.decisions.len() {
            let decision = &self.envelope.audit.decisions[i];
            let entity = match Self::find_entity(&self.envelope.entities, decision) {
                Some(e) => e,
                None => continue,
            };

            let Some(Location::Image(ref loc)) = entity.location else {
                continue;
            };

            let output = match &decision.spec {
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
            let entity_id = decision.entity_id;

            Self::set_replaced_value(
                &mut self.envelope.audit.records,
                entity_id,
                Some("[IMAGE_REDACTED]".into()),
            );

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
        let mut redactions = Vec::new();

        for i in 0..self.envelope.audit.decisions.len() {
            let decision = &self.envelope.audit.decisions[i];
            let entity = match Self::find_entity(&self.envelope.entities, decision) {
                Some(e) => e,
                None => continue,
            };

            let Some(Location::Audio(ref loc)) = entity.location else {
                continue;
            };

            let output = match &decision.spec {
                Strategy::Audio(audio) => match audio {
                    AudioStrategy::Silence => AudioOutput::Silence,
                    AudioStrategy::Remove => AudioOutput::Remove,
                    _ => continue,
                },
                _ => continue,
            };

            let entity_id = decision.entity_id;

            Self::set_replaced_value(
                &mut self.envelope.audit.records,
                entity_id,
                Some("[AUDIO_REDACTED]".into()),
            );

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

    fn find_entity<'b>(
        entities: &'b nvisy_ontology::entity::Entities,
        decision: &RedactionDecision,
    ) -> Option<&'b Entity> {
        entities
            .iter()
            .find(|e| e.source.as_uuid() == decision.entity_id)
    }

    fn set_replaced_value(
        records: &mut [RedactionRecord],
        entity_id: uuid::Uuid,
        value: Option<String>,
    ) {
        if let Some(record) = records.iter_mut().find(|r| r.entity_id == entity_id) {
            record.replaced_value = value;
        }
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
    use nvisy_ontology::provenance::RedactionDecision;

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
        envelope.entities = entities;
        envelope
    }

    #[tokio::test]
    async fn mask_applies_and_records_replacement() {
        let entity = text_entity("John", 0, 6, 10);
        let entity_id = entity.source.as_uuid();
        let decision = RedactionDecision::new(
            entity_id,
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            0.9,
        );
        let record = RedactionRecord::new(entity_id, "John", 0.9);

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.decisions.push(decision);
        envelope.audit.records.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert_eq!(
            envelope.audit.records[0].replaced_value.as_deref(),
            Some("****"),
        );
    }

    #[tokio::test]
    async fn remove_leaves_replaced_value_none() {
        let entity = text_entity("John", 0, 6, 10);
        let entity_id = entity.source.as_uuid();
        let decision = RedactionDecision::new(entity_id, Strategy::Text(TextStrategy::Remove), 0.9);
        let record = RedactionRecord::new(entity_id, "John", 0.9);

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.decisions.push(decision);
        envelope.audit.records.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert!(envelope.audit.records[0].replaced_value.is_none());
    }

    #[tokio::test]
    async fn skips_image_strategy_for_text_entity() {
        let entity = text_entity("face", 0, 0, 4);
        let decision = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Image(ImageStrategy::Blur { sigma: 15.0 }),
            0.9,
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.decisions.push(decision);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();
    }
}
