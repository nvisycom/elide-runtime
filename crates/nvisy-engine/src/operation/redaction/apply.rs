//! Compute replacement text from a strategy and apply redaction
//! decisions to document content via codec transforms.

use nvisy_codec::handler::TextSpanId;
use nvisy_codec::transform::{TextOutput, TextRedaction};
use nvisy_ontology::entity::{Entities, Entity, EntityKind, Location};
use nvisy_ontology::policy::{Strategy, TextStrategy};
use nvisy_ontology::provenance::RedactionDecision;
use sha2::{Digest, Sha256};

const TARGET: &str = "nvisy_engine::op::redaction::apply";

/// Build codec [`TextRedaction`] instructions from redaction decisions
/// and the entities they reference.
///
/// For each decision that targets a text entity with a text strategy,
/// computes the replacement string and emits a `TextRedaction` with
/// the span ID and byte offsets from the entity's [`TextLocation`].
///
/// Decisions targeting image or audio entities are skipped (those
/// need separate codec transforms).
pub(super) fn build_text_redactions(
    decisions: &[RedactionDecision],
    entities: &Entities,
) -> Vec<TextRedaction<TextSpanId>> {
    let mut redactions = Vec::new();

    for decision in decisions {
        let entity = match entities
            .iter()
            .find(|e| e.source.as_uuid() == decision.entity_id)
        {
            Some(e) => e,
            None => continue,
        };

        let Some(Location::Text(ref loc)) = entity.location else {
            continue;
        };

        let replacement = match &decision.spec {
            Strategy::Text(text) => build_text_replacement(entity, text),
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

    tracing::debug!(
        target: TARGET,
        decisions = decisions.len(),
        text_redactions = redactions.len(),
        "built codec instructions",
    );

    redactions
}

/// Compute replacement text for a single entity and text strategy.
fn build_text_replacement(entity: &Entity, strategy: &TextStrategy) -> String {
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

        TextStrategy::Hash => hash_value(&entity.value),

        TextStrategy::Pseudonymize => pseudonymize(&entity.entity_kind, &entity.value),

        TextStrategy::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
        TextStrategy::Generate => format!("[GEN:{}]", entity.entity_kind),
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

    #[test]
    fn mask_builds_text_redaction() {
        let entity = text_entity("John", 0, 5, 9);
        let decision = RedactionDecision::new(
            entity.source.as_uuid(),
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            0.9,
        );
        let entities: Entities = vec![entity].into();
        let redactions = build_text_redactions(&[decision], &entities);
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
        let redactions = build_text_redactions(&[decision], &entities);
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
        let redactions = build_text_redactions(&[decision], &entities);
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
        let redactions = build_text_redactions(&[decision], &entities);
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
        let r1 = build_text_redactions(&[d1.clone()], &entities);
        let r2 = build_text_redactions(&[d1], &entities);
        assert_eq!(
            r1[0].output.replacement_value(),
            r2[0].output.replacement_value()
        );
    }
}
