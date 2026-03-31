//! Compute replacement values and build redaction instructions from
//! evaluated decisions via codec transforms.
//!
//! [`RedactionApplicator`] takes evaluated decisions and the document's
//! entities, and builds per-modality codec instruction vectors. The
//! caller applies them to the document separately (avoiding borrow
//! conflicts with the envelope).

use nvisy_codec::handler::TextSpanId;
use nvisy_codec::transform::{TextOutput, TextRedaction};
use nvisy_ontology::entity::{Entities, Entity, EntityKind, Location};
use nvisy_ontology::policy::{Strategy, TextStrategy};
use nvisy_ontology::provenance::RedactionDecision;
use sha2::{Digest, Sha256};

const TARGET: &str = "nvisy_engine::op::redaction::apply";

/// Builds per-modality codec instructions from redaction decisions
/// and the document's entities.
///
/// The applicator borrows decisions and entities to produce typed
/// redaction instruction vectors. The caller applies them to the
/// document separately (avoiding borrow conflicts with the envelope).
pub(super) struct RedactionApplicator<'a> {
    decisions: &'a [RedactionDecision],
    entities: &'a Entities,
}

impl<'a> RedactionApplicator<'a> {
    pub fn new(decisions: &'a [RedactionDecision], entities: &'a Entities) -> Self {
        Self {
            decisions,
            entities,
        }
    }

    /// Build codec [`TextRedaction`] instructions from decisions targeting
    /// text entities with text strategies.
    pub fn build_text_redactions(&self) -> Vec<TextRedaction<TextSpanId>> {
        let mut redactions = Vec::new();

        for decision in self.decisions {
            let entity = match self.find_entity(decision) {
                Some(e) => e,
                None => continue,
            };

            let Some(Location::Text(ref loc)) = entity.location else {
                continue;
            };

            let replacement = match &decision.spec {
                Strategy::Text(text) => self.text_replacement(entity, text),
                _ => continue,
            };

            let span_id = match &loc.element_id {
                Some(id) => match id.parse::<usize>() {
                    Ok(n) => TextSpanId(n),
                    Err(_) => continue,
                },
                None => continue,
            };

            let output = if replacement.is_empty() {
                TextOutput::Remove
            } else {
                TextOutput::Replace { replacement }
            };

            tracing::trace!(
                target: TARGET,
                entity_id = %decision.entity_id,
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

    /// Build image redaction instructions from decisions targeting
    /// image entities with image strategies.
    pub fn build_image_redactions(&self) -> Vec<nvisy_codec::transform::ImageRedaction> {
        use nvisy_codec::transform::{ImageOutput, ImageRedaction};
        use nvisy_ontology::policy::ImageStrategy;

        let mut redactions = Vec::new();

        for decision in self.decisions {
            let entity = match self.find_entity(decision) {
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

            tracing::trace!(
                target: TARGET,
                entity_id = %decision.entity_id,
                "built image redaction instruction",
            );

            redactions.push(ImageRedaction {
                bounding_box: loc.bounding_box,
                output,
            });
        }

        redactions
    }

    /// Build audio redaction instructions from decisions targeting
    /// audio entities with audio strategies.
    pub fn build_audio_redactions(&self) -> Vec<nvisy_codec::transform::AudioRedaction> {
        use nvisy_codec::transform::{AudioOutput, AudioRedaction};
        use nvisy_ontology::policy::AudioStrategy;

        let mut redactions = Vec::new();

        for decision in self.decisions {
            let entity = match self.find_entity(decision) {
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

            tracing::trace!(
                target: TARGET,
                entity_id = %decision.entity_id,
                start = loc.time_span.start_secs,
                end = loc.time_span.end_secs,
                "built audio redaction instruction",
            );

            redactions.push(AudioRedaction {
                start_secs: loc.time_span.start_secs,
                end_secs: loc.time_span.end_secs,
                output,
            });
        }

        redactions
    }

    fn find_entity(&self, decision: &RedactionDecision) -> Option<&'a Entity> {
        self.entities
            .iter()
            .find(|e| e.source.as_uuid() == decision.entity_id)
    }

    /// Compute replacement text for a single entity and text strategy.
    fn text_replacement(&self, entity: &Entity, strategy: &TextStrategy) -> String {
        match strategy {
            TextStrategy::Mask { mask_char } => mask_char.to_string().repeat(entity.value.len()),

            TextStrategy::Replace { placeholder } => {
                if placeholder.is_empty() {
                    format!("[{}]", entity.entity_kind.to_string().to_uppercase())
                } else {
                    placeholder
                        .replace("{entityType}", &entity.entity_kind.to_string())
                        .replace("{category}", &entity.category.to_string())
                }
            }

            TextStrategy::Remove => String::new(),

            TextStrategy::Hash => Self::hash_value(&entity.value),

            TextStrategy::Pseudonymize => Self::pseudonymize(&entity.entity_kind, &entity.value),

            TextStrategy::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
            TextStrategy::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_kind),
            _ => format!("[REDACTED:{}]", entity.entity_kind),
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
    use nvisy_ontology::entity::{
        Entity, EntityCategory, EntityKind, RecognitionMethod, TextLocation,
    };

    use super::*;

    fn text_entity(value: &str, span_id: usize, start: usize, end: usize) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_value(value)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .with_location(Location::from(TextLocation {
                start_offset: start,
                end_offset: end,
                element_id: Some(span_id.to_string()),
                ..Default::default()
            }))
            .build()
            .unwrap()
    }

    fn applicator<'a>(
        decisions: &'a [RedactionDecision],
        entities: &'a Entities,
    ) -> RedactionApplicator<'a> {
        RedactionApplicator::new(decisions, entities)
    }

    #[test]
    fn mask_builds_text_redaction() {
        let entity = text_entity("John", 0, 5, 9);
        let decision = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            0.9,
        );
        let entities: Entities = vec![entity].into();
        let redactions = applicator(&[decision], &entities).build_text_redactions();
        assert_eq!(redactions.len(), 1);
        assert_eq!(redactions[0].span_id, TextSpanId(0));
        assert_eq!(redactions[0].start, 5);
        assert_eq!(redactions[0].end, 9);
        assert_eq!(redactions[0].output.replacement_value(), Some("****"));
    }

    #[test]
    fn remove_builds_remove_output() {
        let entity = text_entity("secret", 0, 0, 6);
        let decision = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Text(TextStrategy::Remove),
            0.9,
        );
        let entities: Entities = vec![entity].into();
        let redactions = applicator(&[decision], &entities).build_text_redactions();
        assert_eq!(redactions.len(), 1);
        assert!(redactions[0].output.replacement_value().is_none());
    }

    #[test]
    fn skips_image_strategy() {
        use nvisy_ontology::policy::ImageStrategy;

        let entity = text_entity("face", 0, 0, 4);
        let decision = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Image(ImageStrategy::Blur { sigma: 15.0 }),
            0.9,
        );
        let entities: Entities = vec![entity].into();
        let redactions = applicator(&[decision], &entities).build_text_redactions();
        assert!(redactions.is_empty());
    }

    #[test]
    fn skips_entity_without_location() {
        let entity = Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_value("John")
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(0.9)
            .build()
            .unwrap();
        let decision = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Text(TextStrategy::Hash),
            0.9,
        );
        let entities: Entities = vec![entity].into();
        let redactions = applicator(&[decision], &entities).build_text_redactions();
        assert!(redactions.is_empty());
    }

    #[test]
    fn hash_replacement_is_deterministic() {
        let entity = text_entity("John Smith", 0, 0, 10);
        let d1 = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Text(TextStrategy::Hash),
            0.9,
        );
        let entities: Entities = vec![entity].into();
        let r1 = applicator(&[d1.clone()], &entities).build_text_redactions();
        let r2 = applicator(&[d1], &entities).build_text_redactions();
        assert_eq!(
            r1[0].output.replacement_value(),
            r2[0].output.replacement_value()
        );
    }
}
