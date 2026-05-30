//! Group + fuse co-referent entities into a single detection.
//!
//! Implements the actual confidence-combination algorithms and entity
//! field merging. The strategy determines *how* confidences are
//! combined; the grouping module determines *which* entities are
//! candidates.

mod group;
mod key;
mod strategy;

use std::cmp::Ordering;
use std::collections::HashSet;

use nvisy_ontology::entity::{Entity, RefinementMethod};
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::primitive::Confidence;

use self::group::GroupEntities;
pub use self::group::GroupingCriteria;
pub use self::strategy::DeduplicationStrategy;
use super::span_size::SpanSize;
use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;

const TARGET: &str = "nvisy_engine::op::deduplication::fuse";

/// Extension trait: group co-referent entities by `criteria` then
/// merge each group into a single entity according to `strategy`.
/// Mutates in place.
pub(super) trait Fuse<M: Modality> {
    /// Group + fuse the entity collection in place.
    fn fuse(
        &mut self,
        strategy: &DeduplicationStrategy,
        criteria: GroupingCriteria,
        envelope: &DocumentEnvelope<M>,
    ) -> impl Future<Output = ()> + Send;
}

impl<M> Fuse<M> for Vec<Entity<M>>
where
    M: Modality + Overlap + SpanSize,
    DocumentEnvelope<M>: ValueAt<M>,
{
    async fn fuse(
        &mut self,
        strategy: &DeduplicationStrategy,
        criteria: GroupingCriteria,
        envelope: &DocumentEnvelope<M>,
    ) {
        if self.len() <= 1 {
            return;
        }

        let entities = std::mem::take(self);
        let groups = entities.group(criteria, envelope).await;

        tracing::debug!(
            target: TARGET,
            strategy = ?strategy,
            groups = groups.len(),
            "fusing entity groups",
        );

        for group in groups {
            self.push(fuse_group(strategy, group, envelope).await);
        }
    }
}

/// Fuse a group of co-referent entities into one.
async fn fuse_group<M>(
    strategy: &DeduplicationStrategy,
    mut group: Vec<Entity<M>>,
    envelope: &DocumentEnvelope<M>,
) -> Entity<M>
where
    M: Modality + SpanSize,
    DocumentEnvelope<M>: ValueAt<M>,
{
    debug_assert!(!group.is_empty());

    if group.len() == 1 {
        return group
            .into_iter()
            .next()
            .expect("group.len() == 1 by guard above");
    }

    let fused_confidence = strategy.compute_confidence(&group);

    // Determine the refinement type: if all entities in the group
    // share the same set of recognition method kinds, this is a
    // deduplication (same detector produced duplicates). Otherwise
    // it's an ensemble fusion (different detectors combined).
    let refinement = classify_refinement(&group);

    // Sort by descending confidence: highest-confidence entity
    // becomes the base since it carries the most trusted metadata.
    group.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(Ordering::Equal)
    });
    let mut result = group.remove(0);
    let rest = group;

    // Prefer the largest span: for text, the longer match is more
    // specific (e.g. "John Smith" over "John"); for images, the
    // larger bounding box. Adopt the winner's location so the
    // span stays consistent.
    for e in &rest {
        if e.location
            .span_cmp(&result.location)
            .unwrap_or(Ordering::Less)
            == Ordering::Greater
        {
            result.location.clone_from(&e.location);
        }
    }

    // Merge recognition methods (order-preserving union).
    let mut seen_rec: HashSet<_> = result.recognition_methods.iter().cloned().collect();
    for e in &rest {
        for m in &e.recognition_methods {
            if seen_rec.insert(m.clone()) {
                result.recognition_methods.push(m.clone());
            }
        }
    }

    // Fill in missing optional fields from lower-confidence entities.
    if result.language.is_none() {
        result.language = rest.iter().find_map(|e| e.language.clone());
    }
    result.confidence =
        Confidence::new(fused_confidence.clamp(0.0, 1.0)).expect("clamped to [0,1]");
    result.refinement_methods.push(refinement);

    let value = envelope
        .value_at(&result.location)
        .await
        .unwrap_or_default();
    tracing::trace!(
        target: TARGET,
        entity_id = %result.id,
        fused_from = rest.len() + 1,
        confidence = fused_confidence,
        ?refinement,
        value,
        "fused entity group",
    );

    result
}

/// Classify how the group was formed (all same detector kind → Dedup,
/// mixed → Ensemble).
fn classify_refinement<M: Modality>(group: &[Entity<M>]) -> RefinementMethod {
    let first_kinds: HashSet<_> = group[0]
        .recognition_methods
        .iter()
        .map(|m| m.kind())
        .collect();
    let all_same = group.iter().skip(1).all(|e| {
        e.recognition_methods
            .iter()
            .map(|m| m.kind())
            .collect::<HashSet<_>>()
            == first_kinds
    });
    if all_same {
        RefinementMethod::Deduplication
    } else {
        RefinementMethod::EnsembleFusion
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use nvisy_core::content::ContentMetadata;
    use nvisy_ontology::entity::{Entity, ModelKind, RecognitionMethod, RecognitionMethodKind};
    use nvisy_ontology::primitive::Confidence;
    use tokio::sync::Mutex;

    use super::*;
    use crate::envelope::SharedData;

    async fn test_envelope(text: &str) -> DocumentEnvelope<nvisy_ontology::modality::Text> {
        let registry = crate::ingestion::registry::Registry::open(
            tempfile::tempdir().expect("tempdir").path(),
        )
        .expect("open registry");
        let shared = SharedData::new(uuid::Uuid::nil(), uuid::Uuid::nil(), registry);
        let handle = nvisy_formats::test_utils::decode_text(text)
            .await
            .expect("decode text");
        DocumentEnvelope::<nvisy_ontology::modality::Text>::new(
            Arc::new(Mutex::new(handle)),
            ContentMetadata::new().with_content_type("text/plain"),
            nvisy_ontology::modality::TextMetadata {
                extraction: nvisy_ontology::modality::TextExtraction::Native,
                languages: Vec::new(),
            },
            shared,
        )
        .await
    }

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    const TEXT: &str = "John Smith";

    #[tokio::test]
    async fn strict_groups_exact_overlap() {
        let doc = test_envelope(TEXT).await;
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4).test_build(),
        ];
        entities
            .fuse(
                &DeduplicationStrategy::MaxConfidence,
                GroupingCriteria::Strict,
                &doc,
            )
            .await;
        assert_eq!(entities.len(), 1);
        assert!((entities[0].confidence.get() - 0.9).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn narrowing_groups_substring_with_overlap() {
        let doc = test_envelope(TEXT).await;
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 10)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ];
        entities
            .fuse(
                &DeduplicationStrategy::MaxConfidence,
                GroupingCriteria::Narrowing,
                &doc,
            )
            .await;
        assert_eq!(entities.len(), 1);
        let value = doc.value_at(&entities[0].location).await;
        assert_eq!(value.as_deref(), Some("John Smith"));
    }

    #[tokio::test]
    async fn widening_groups_across_non_overlapping_locations() {
        let text = format!("{:<100}John Smith", TEXT);
        let doc = test_envelope(&text).await;
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(100, 110)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ];
        entities
            .fuse(
                &DeduplicationStrategy::MaxConfidence,
                GroupingCriteria::Widening,
                &doc,
            )
            .await;
        assert_eq!(entities.len(), 1);
    }

    #[tokio::test]
    async fn noisy_or_strategy() {
        let doc = test_envelope(TEXT).await;
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.7))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        entities
            .fuse(
                &DeduplicationStrategy::NoisyOr,
                GroupingCriteria::default(),
                &doc,
            )
            .await;
        assert_eq!(entities.len(), 1);
        // 1 - (1 - 0.7)(1 - 0.8) = 1 - 0.06 = 0.94
        assert!((entities[0].confidence.get() - 0.94).abs() < 0.001);
    }

    #[tokio::test]
    async fn weighted_average_strategy() {
        let doc = test_envelope(TEXT).await;
        let mut weights = HashMap::new();
        weights.insert(RecognitionMethodKind::Pattern, 1.0);
        weights.insert(RecognitionMethodKind::NlpNer, 2.0);

        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.6))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ];
        entities
            .fuse(
                &DeduplicationStrategy::WeightedAverage { weights },
                GroupingCriteria::default(),
                &doc,
            )
            .await;
        assert_eq!(entities.len(), 1);
        // (0.6 * 1.0 + 0.9 * 2.0) / 3.0 = 0.8
        assert!((entities[0].confidence.get() - 0.8).abs() < 0.001);
    }

    #[tokio::test]
    async fn different_detector_tagged_as_ensemble_fusion() {
        let doc = test_envelope(TEXT).await;
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ];
        entities
            .fuse(
                &DeduplicationStrategy::MaxConfidence,
                GroupingCriteria::default(),
                &doc,
            )
            .await;
        assert_eq!(entities.len(), 1);
        assert!(
            entities[0]
                .refinement_methods
                .contains(&RefinementMethod::EnsembleFusion)
        );
    }
}
