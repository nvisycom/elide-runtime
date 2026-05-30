//! Shared LLM-verification output shape.
//!
//! Both [`CvVerifyAgent`] and [`NerVerifyAgent`] prompt an LLM with a list
//! of proposed entities and ask it to vote confirm/correct/reject
//! per entry. The verdict shape is identical across modalities;
//! only the per-modality location update (bounding box vs. text
//! offsets) differs, and that is each verifier's concern when
//! applying a verdict to an entity.
//!
//! Confirmed entities are omitted from [`VerificationOutput`]; only
//! changed (corrected or rejected) entries appear.
//!
//! [`CvVerifyAgent`]: crate::agent::cv::CvVerifyAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent

use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RefinementMethod};
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::{BoundingBox, Confidence};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether a proposed entity was corrected or rejected by the
/// verifier LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The entity value or classification was corrected.
    Corrected,
    /// The entity was rejected as a false positive.
    Rejected,
}

/// A single entity whose status changed during LLM verification.
///
/// The `id` field is the index into the proposed entity slice, so
/// the caller can diff against the original list.
///
/// Modality-specific fields (`bbox` for CV, future text fields for
/// NER) are optional — the verifier only fills in what its modality
/// supports.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct VerifiedEntity {
    /// Index into the proposed entity list.
    pub id: usize,
    /// Whether this entity was corrected or rejected.
    pub status: VerificationStatus,
    /// Corrected category (present when `status` is `Corrected`).
    pub category: Option<EntityCategory>,
    /// Corrected entity type (present when `status` is `Corrected`).
    pub entity_type: Option<EntityKind>,
    /// Corrected value (present when `status` is `Corrected`).
    pub value: Option<String>,
    /// Verifier confidence in the verdict.
    pub confidence: Confidence,
    /// Corrected bounding box (CV verifier only; present when
    /// `status` is `Corrected` and the modality is image-based).
    pub bbox: Option<BoundingBox>,
    /// Optional rationale for the correction or rejection.
    pub reason: Option<String>,
}

impl VerifiedEntity {
    /// Apply this verdict to a text-modality entity, returning the
    /// adjusted entity for `Corrected` and `None` for `Rejected`.
    ///
    /// For `Corrected`: optionally overrides `category` and
    /// `entity_kind` when the verdict carries them, always updates
    /// `confidence`, and stamps
    /// [`RefinementMethod::ModelVerification`] (idempotent). The
    /// entity's `location` and surface value are frozen — they're
    /// determined by the original recognizer, not the verifier.
    /// `value` and `bbox` fields in the verdict are ignored on
    /// this path; image-modality has its own counterpart.
    pub fn apply_to_text(&self, mut entity: Entity<Text>) -> Option<Entity<Text>> {
        match self.status {
            VerificationStatus::Rejected => None,
            VerificationStatus::Corrected => {
                if let Some(cat) = self.category {
                    entity.category = cat;
                }
                if let Some(kind) = self.entity_type {
                    entity.entity_kind = kind;
                }
                // Clamp into [0, 1] defensively — Confidence::new
                // already returned the verdict's value, so this is
                // only here in case schema drift loosens the type.
                if let Some(c) = Confidence::new(self.confidence.get().clamp(0.0, 1.0)) {
                    entity.confidence = c;
                }
                if !entity
                    .refinement_methods
                    .contains(&RefinementMethod::ModelVerification)
                {
                    entity
                        .refinement_methods
                        .push(RefinementMethod::ModelVerification);
                }
                Some(entity)
            }
        }
    }
}

/// Verification output containing only entities whose status
/// changed.
///
/// Entities not present in this list are implicitly confirmed.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VerificationOutput {
    /// Only entities that were corrected or rejected.
    pub entities: Vec<VerifiedEntity>,
}

impl VerificationOutput {
    /// Apply every verdict in this output to the matching entity
    /// in `entities` (matched by index — the LLM uses position in
    /// the verify prompt as the verdict key).
    ///
    /// Entities absent from the verdict list are implicitly
    /// confirmed and pass through unchanged. Per-entity handling
    /// — drop on `Rejected`, mutate on `Corrected` — lives on
    /// [`VerifiedEntity::apply_to_text`]. Out-of-range indices in
    /// the verdict list are dropped and counted into the returned
    /// `dropped_oor` field so callers can log a diagnostic.
    /// Duplicate indices: the last verdict in the list wins.
    pub fn apply_to_text(
        self,
        entities: Vec<Entity<Text>>,
    ) -> VerificationApplyOutcome {
        use std::collections::HashMap;

        let verdicts: HashMap<usize, VerifiedEntity> =
            self.entities.into_iter().map(|v| (v.id, v)).collect();
        let total = entities.len();
        let dropped_oor = verdicts.keys().filter(|&&i| i >= total).count();

        let survivors = entities
            .into_iter()
            .enumerate()
            .filter_map(|(i, entity)| match verdicts.get(&i) {
                None => Some(entity),
                Some(verdict) => verdict.apply_to_text(entity),
            })
            .collect();

        VerificationApplyOutcome {
            survivors,
            dropped_oor,
        }
    }
}

/// Result of applying a [`VerificationOutput`] to a list of
/// entities: the survivors plus telemetry the caller can log.
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationApplyOutcome {
    /// Entities that survived the verdict pass (confirmed +
    /// corrected, with corrections applied).
    pub survivors: Vec<Entity<Text>>,
    /// Count of verdicts whose `id` referred to an index past
    /// `entities.len()`. The verifier shouldn't emit these; when
    /// it does, the caller typically logs it at debug.
    pub dropped_oor: usize,
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{
        ModelKind, ModelProvenance, RecognitionMethod,
    };

    use super::*;

    fn entity(start: usize, end: usize, kind: EntityKind) -> Entity<Text> {
        Entity::builder()
            .with_category(kind.category())
            .with_entity_kind(kind)
            .with_recognition_methods(vec![RecognitionMethod::LlmNer(ModelProvenance::new(
                "test",
                ModelKind::Gateway,
            ))])
            .with_confidence(Confidence::new(0.5).unwrap())
            .with_location(Text::new(start, end))
            .build()
            .unwrap()
    }

    fn verdict(
        id: usize,
        status: VerificationStatus,
        confidence: f64,
        kind: Option<EntityKind>,
        category: Option<EntityCategory>,
    ) -> VerifiedEntity {
        VerifiedEntity {
            id,
            status,
            category,
            entity_type: kind,
            value: None,
            confidence: Confidence::new(confidence).unwrap(),
            bbox: None,
            reason: None,
        }
    }

    #[test]
    fn absent_verdict_means_confirm() {
        let entities = vec![entity(0, 5, EntityKind::PersonName)];
        let out = VerificationOutput::default().apply_to_text(entities);
        assert_eq!(out.survivors.len(), 1);
        assert!(out.survivors[0].refinement_methods.is_empty());
        assert_eq!(out.dropped_oor, 0);
    }

    #[test]
    fn rejected_verdict_drops_entity() {
        let entities = vec![entity(0, 5, EntityKind::PersonName)];
        let out = VerificationOutput {
            entities: vec![verdict(0, VerificationStatus::Rejected, 0.1, None, None)],
        }
        .apply_to_text(entities);
        assert!(out.survivors.is_empty());
    }

    #[test]
    fn corrected_verdict_updates_confidence_and_stamps_refinement() {
        let entities = vec![entity(0, 5, EntityKind::PersonName)];
        let out = VerificationOutput {
            entities: vec![verdict(0, VerificationStatus::Corrected, 0.9, None, None)],
        }
        .apply_to_text(entities);
        assert_eq!(out.survivors.len(), 1);
        assert!((out.survivors[0].confidence.get() - 0.9).abs() < f64::EPSILON);
        assert!(
            out.survivors[0]
                .refinement_methods
                .contains(&RefinementMethod::ModelVerification)
        );
    }

    #[test]
    fn corrected_verdict_can_override_kind_and_category() {
        let entities = vec![entity(0, 5, EntityKind::PersonName)];
        let out = VerificationOutput {
            entities: vec![verdict(
                0,
                VerificationStatus::Corrected,
                0.9,
                Some(EntityKind::Age),
                Some(EntityCategory::Demographic),
            )],
        }
        .apply_to_text(entities);
        assert_eq!(out.survivors.len(), 1);
        assert_eq!(out.survivors[0].entity_kind, EntityKind::Age);
        assert_eq!(out.survivors[0].category, EntityCategory::Demographic);
    }

    #[test]
    fn out_of_range_verdict_is_counted_and_dropped() {
        let entities = vec![entity(0, 5, EntityKind::PersonName)];
        let out = VerificationOutput {
            entities: vec![verdict(99, VerificationStatus::Rejected, 0.1, None, None)],
        }
        .apply_to_text(entities);
        assert_eq!(out.survivors.len(), 1);
        assert_eq!(out.dropped_oor, 1);
    }

    #[test]
    fn correction_does_not_double_stamp_refinement_method() {
        let mut e = entity(0, 5, EntityKind::PersonName);
        e.refinement_methods
            .push(RefinementMethod::ModelVerification);
        let out = VerificationOutput {
            entities: vec![verdict(0, VerificationStatus::Corrected, 0.9, None, None)],
        }
        .apply_to_text(vec![e]);
        assert_eq!(out.survivors.len(), 1);
        let count = out.survivors[0]
            .refinement_methods
            .iter()
            .filter(|m| matches!(m, RefinementMethod::ModelVerification))
            .count();
        assert_eq!(count, 1);
    }
}
