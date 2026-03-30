//! Entity fusion operation.
//!
//! Runs at **phase 3**, after detection. Combines calibration,
//! deduplication, and ensemble fusion into a single operation.
//!
//! The pipeline is:
//! 1. **Calibrate** — scale raw confidence per recognition method.
//! 2. **Deduplicate** — merge exact overlapping duplicates (always on).
//! 3. **Fuse** — combine multi-detector groups via the configured strategy.

use std::collections::{HashMap, HashSet};

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity, EntityKind, Overlap, RefinementMethod};
use nvisy_ontology::workflow::{CalibrationMap, Fusion, FusionStrategy, GroupingCriteria};

use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::operation::envelope::RefinedEntities;

const TARGET: &str = "nvisy_engine::op::fusion";

/// Hash key for the first grouping phase.
///
/// For `Strict` and `Narrowing`/`Widening` this is `(kind, exact_value)`;
/// for `Normalized` it's `(kind, lowercased_trimmed_value)`.
///
/// Substring-based criteria (`Narrowing`/`Widening`) use exact keys in
/// the hash phase, then do pairwise substring checks in the overlap phase.
#[derive(Hash, PartialEq, Eq)]
struct GroupKey {
    kind: EntityKind,
    value: String,
}

/// Check whether two values match under the given criteria.
fn values_match(a: &str, b: &str, criteria: GroupingCriteria) -> bool {
    match criteria {
        GroupingCriteria::Strict => a == b,
        GroupingCriteria::Normalized => a.trim().eq_ignore_ascii_case(b.trim()),
        GroupingCriteria::Narrowing | GroupingCriteria::Widening => {
            let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
            long.contains(short)
        }
        _ => a == b,
    }
}

/// Whether the criteria requires overlapping locations.
fn requires_location_overlap(criteria: GroupingCriteria) -> bool {
    !matches!(criteria, GroupingCriteria::Widening)
}

/// Normalise a value for HashMap bucketing.
fn bucket_value(value: &str, criteria: GroupingCriteria) -> String {
    match criteria {
        GroupingCriteria::Normalized => value.trim().to_lowercase(),
        _ => value.to_owned(),
    }
}

/// Group entities using a two-phase approach:
///
/// 1. **Hash phase** — bucket by `(kind, normalized_value)` via HashMap.
/// 2. **Merge phase** — within each bucket, merge groups by value match
///    + optional location overlap.
///
/// For substring criteria (Narrowing/Widening), entities with different
/// exact values land in different buckets. A post-hash cross-bucket merge
/// handles substring containment within the same EntityKind.
fn group_entities(entities: Entities, criteria: GroupingCriteria) -> Vec<Vec<Entity>> {
    let check_overlap = requires_location_overlap(criteria);
    let is_substring = matches!(
        criteria,
        GroupingCriteria::Narrowing | GroupingCriteria::Widening
    );

    // Phase 1: bucket by (kind, value).
    let mut buckets: HashMap<GroupKey, Vec<Entity>> = HashMap::new();
    for entity in entities {
        let key = GroupKey {
            kind: entity.entity_kind,
            value: bucket_value(&entity.value, criteria),
        };
        buckets.entry(key).or_default().push(entity);
    }

    // Phase 2a: within each bucket, sub-group by location overlap.
    let mut groups: Vec<Vec<Entity>> = Vec::new();
    let mut kind_groups: HashMap<EntityKind, Vec<usize>> = HashMap::new();

    for (_key, bucket) in buckets {
        let kind = bucket[0].entity_kind;
        let mut sub_groups: Vec<Vec<Entity>> = Vec::new();

        for entity in bucket {
            let target = sub_groups
                .iter_mut()
                .find(|g| !check_overlap || g[0].location.overlaps(&entity.location));
            match target {
                Some(g) => g.push(entity),
                None => sub_groups.push(vec![entity]),
            }
        }

        for sg in sub_groups {
            let idx = groups.len();
            groups.push(sg);
            if is_substring {
                kind_groups.entry(kind).or_default().push(idx);
            }
        }
    }

    // Phase 2b: for substring criteria, merge groups within the same kind
    // whose values have a containment relationship.
    if is_substring {
        for indices in kind_groups.values() {
            let mut merged_into: HashSet<usize> = HashSet::new();
            for i in 0..indices.len() {
                if merged_into.contains(&indices[i]) {
                    continue;
                }
                for j in (i + 1)..indices.len() {
                    if merged_into.contains(&indices[j]) {
                        continue;
                    }
                    let val_a = &groups[indices[i]][0].value;
                    let val_b = &groups[indices[j]][0].value;
                    let location_ok = !check_overlap
                        || groups[indices[i]][0]
                            .location
                            .overlaps(&groups[indices[j]][0].location);
                    if location_ok && values_match(val_a, val_b, criteria) {
                        let donor = std::mem::take(&mut groups[indices[j]]);
                        groups[indices[i]].extend(donor);
                        merged_into.insert(indices[j]);
                    }
                }
            }
        }
        groups.retain(|g| !g.is_empty());
    }

    groups
}

/// Apply per-method calibration multipliers to entity confidences.
fn calibrate(entities: &mut Entities, calibration: &CalibrationMap) {
    for entity in entities.iter_mut() {
        let multiplier = entity
            .recognition_methods
            .iter()
            .filter_map(|m| calibration.get(m).copied())
            .reduce(f64::max);
        if let Some(m) = multiplier {
            entity.confidence = (entity.confidence * m).clamp(0.0, 1.0);
        }
    }
}

/// Execution behavior for [`FusionStrategy`].
trait FusionStrategyExt {
    /// Group entities then fuse each group into a single entity.
    fn fuse(&self, entities: Entities, criteria: GroupingCriteria) -> Entities;

    /// Fuse a group of matching entities into a single entity.
    fn fuse_group(&self, group: Vec<Entity>) -> Entity;

    /// Compute fused confidence for a group of entities.
    fn compute_confidence(&self, group: &[Entity]) -> f64;
}

impl FusionStrategyExt for FusionStrategy {
    fn fuse(&self, entities: Entities, criteria: GroupingCriteria) -> Entities {
        if entities.len() <= 1 {
            return entities;
        }

        group_entities(entities, criteria)
            .into_iter()
            .map(|group| self.fuse_group(group))
            .collect()
    }

    fn fuse_group(&self, mut group: Vec<Entity>) -> Entity {
        debug_assert!(!group.is_empty());

        if group.len() == 1 {
            return group.into_iter().next().unwrap();
        }

        let fused_confidence = self.compute_confidence(&group);

        // Pick the highest-confidence entity as the base.
        group.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut result = group.remove(0);
        let rest = group;

        // Use the longest value (more specific match).
        for e in &rest {
            if e.value.len() > result.value.len() {
                result.value.clone_from(&e.value);
                result.location.clone_from(&e.location);
            }
        }

        // Merge recognition methods (order-preserving dedup).
        let mut seen_rec: HashSet<_> = result.recognition_methods.iter().copied().collect();
        for e in &rest {
            for m in &e.recognition_methods {
                if seen_rec.insert(*m) {
                    result.recognition_methods.push(*m);
                }
            }
        }

        // Merge extraction methods.
        let mut seen_ext: HashSet<_> = result.extraction_methods.iter().copied().collect();
        for e in &rest {
            for m in &e.extraction_methods {
                if seen_ext.insert(*m) {
                    result.extraction_methods.push(*m);
                }
            }
        }

        // Fill in missing optional fields from other entities.
        if result.language.is_none() {
            result.language = rest.iter().find_map(|e| e.language.clone());
        }
        if result.model.is_none() {
            result.model = rest.iter().find_map(|e| e.model.clone());
        }

        result.confidence = fused_confidence;
        result
            .refinement_methods
            .push(RefinementMethod::EnsembleFusion);
        result
    }

    fn compute_confidence(&self, group: &[Entity]) -> f64 {
        match self {
            Self::MaxConfidence => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
            Self::WeightedAverage { weights } => {
                let mut total_weight = 0.0;
                let mut weighted_sum = 0.0;
                for e in group {
                    let w = e
                        .recognition_methods
                        .iter()
                        .filter_map(|m| weights.get(m).copied())
                        .fold(1.0_f64, f64::max);
                    weighted_sum += e.confidence * w;
                    total_weight += w;
                }
                if total_weight > 0.0 {
                    weighted_sum / total_weight
                } else {
                    0.0
                }
            }
            Self::NoisyOr => {
                let product: f64 = group.iter().map(|e| 1.0 - e.confidence).product();
                1.0 - product
            }
            _ => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
        }
    }
}

/// Combined calibration + deduplication + ensemble fusion operation.
pub struct FusionOp {
    grouping: GroupingCriteria,
    strategy: FusionStrategy,
    calibration: CalibrationMap,
}

impl FusionOp {
    /// Create from graph config.
    pub fn new(cfg: &Fusion) -> Self {
        Self {
            grouping: cfg.grouping,
            strategy: cfg.strategy.clone(),
            calibration: cfg.calibration.clone(),
        }
    }

    pub(crate) fn execute(&self, mut entities: Entities) -> RefinedEntities {
        if entities.is_empty() {
            return RefinedEntities(entities);
        }

        let before = entities.len();

        // Phase 1: calibrate raw confidence scores.
        if !self.calibration.is_empty() {
            calibrate(&mut entities, &self.calibration);
            tracing::debug!(
                target: TARGET,
                methods = self.calibration.len(),
                "applied confidence calibration",
            );
        }

        // Phase 2: deduplicate + fuse.
        let result = self.strategy.fuse(entities, self.grouping);

        tracing::debug!(
            target: TARGET,
            before,
            after_fusion = result.len(),
            "fusion complete",
        );

        RefinedEntities(result)
    }
}

impl Operation for FusionOp {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<RefinedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|data| async move { Ok(self.execute(data)) })
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nvisy_ontology::entity::{
        EntityCategory, EntityKind, ExtractionMethod, RecognitionMethod, TextLocation,
    };
    use nvisy_ontology::workflow::FusionStrategy::*;

    use super::*;

    fn text_entity(
        value: &str,
        method: RecognitionMethod,
        confidence: f64,
        start: usize,
        end: usize,
    ) -> Entity {
        Entity::new(
            EntityCategory::PersonalIdentity,
            EntityKind::PersonName,
            value,
            method,
            confidence,
        )
        .with_location(
            TextLocation {
                start_offset: start,
                end_offset: end,
                ..Default::default()
            }
            .into(),
        )
    }

    #[test]
    fn strict_groups_exact_overlap() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.9, 0, 4),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn strict_preserves_non_overlapping() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.9, 10, 14),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn normalized_groups_case_insensitive() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("john", RecognitionMethod::Ner, 0.9, 0, 4),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Normalized);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn narrowing_groups_substring_with_overlap() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John Smith", RecognitionMethod::Ner, 0.9, 0, 10),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Narrowing);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John Smith");
    }

    #[test]
    fn narrowing_preserves_non_overlapping_substrings() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John Smith", RecognitionMethod::Ner, 0.9, 100, 110),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Narrowing);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn widening_groups_across_locations() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John Smith", RecognitionMethod::Ner, 0.9, 100, 110),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Widening);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John Smith");
    }

    #[test]
    fn max_confidence_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.85, 0, 4),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn noisy_or_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.8, 0, 4),
        ]
        .into();
        let result = NoisyOr.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.94).abs() < 0.001);
    }

    #[test]
    fn weighted_average_strategy() {
        let mut weights = HashMap::new();
        weights.insert(RecognitionMethod::Regex, 1.0);
        weights.insert(RecognitionMethod::Ner, 2.0);

        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.6, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.9, 0, 4),
        ]
        .into();
        let result = WeightedAverage { weights }.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn full_pipeline() {
        let cfg = Fusion {
            strategy: FusionStrategy::MaxConfidence,
            ..Default::default()
        };
        let fusion = FusionOp::new(&cfg);
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.85, 0, 4),
        ]
        .into();

        let result = fusion.execute(entities);
        assert_eq!(result.0.len(), 1);
        assert!((result.0[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_input() {
        let cfg = Fusion::default();
        let fusion = FusionOp::new(&cfg);
        let result = fusion.execute(Entities::new());
        assert!(result.0.is_empty());
    }

    #[test]
    fn calibration_scales_confidence() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethod::Regex, 0.5);

        let mut entities: Entities =
            vec![text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4)].into();
        calibrate(&mut entities, &calibration);
        assert!((entities[0].confidence - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn calibration_clamps_to_one() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethod::Regex, 2.0);

        let mut entities: Entities =
            vec![text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4)].into();
        calibrate(&mut entities, &calibration);
        assert!((entities[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuse_picks_longest_value() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.9, 0, 4),
            text_entity("John Smith", RecognitionMethod::Ner, 0.7, 0, 10),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Widening);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John Smith");
    }

    #[test]
    fn fuse_merges_extraction_methods() {
        let mut e1 = text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4);
        e1.extraction_methods = vec![ExtractionMethod::DocumentParsing];
        let mut e2 = text_entity("John", RecognitionMethod::Ner, 0.9, 0, 4);
        e2.extraction_methods = vec![ExtractionMethod::OpticalCharacterRecognition];

        let entities: Entities = vec![e1, e2].into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result[0].extraction_methods.len(), 2);
    }

    #[test]
    fn fuse_fills_missing_language() {
        let mut e1 = text_entity("John", RecognitionMethod::Regex, 0.9, 0, 4);
        let e2 = text_entity("John", RecognitionMethod::Ner, 0.7, 0, 4);
        e1.language = None;
        let mut e2 = e2;
        e2.language = Some("en".into());

        let entities: Entities = vec![e1, e2].into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result[0].language.as_deref(), Some("en"));
    }
}
