//! Entity deduplication operation.
//!
//! Runs at **phase 3**, after detection. Combines multiple detection
//! passes into a single, deduplicated set of entities with combined
//! confidence scores.
//!
//! The deduplication pipeline:
//!
//! 1. **Calibrate**: scale raw confidence scores using per-method
//!    multipliers from [`CalibrationMap`].
//! 2. **Group + fuse**: partition entities by kind, value, and
//!    location overlap (per [`GroupingCriteria`]), then combine each
//!    group into one entity using the [`DeduplicationStrategy`].
//! 3. **Conflict resolution**: resolve cross-kind span overlaps per
//!    the configured [`ConflictResolution`] strategy.
//! 4. **Threshold filter**: drop entities below the minimum
//!    confidence threshold.
//!
//! [`CalibrationMap`]: nvisy_ontology::workflow::CalibrationMap
//! [`GroupingCriteria`]: nvisy_ontology::workflow::GroupingCriteria
//! [`DeduplicationStrategy`]: nvisy_ontology::workflow::DeduplicationStrategy
//! [`ConflictResolution`]: nvisy_ontology::workflow::ConflictResolution

mod calibration;
mod conflict;
mod grouping;
pub(crate) mod span_size;
mod strategy;

use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::workflow::{
    CalibrationMap, ConflictResolution, Deduplication, DeduplicationStrategy, GroupingCriteria,
};

use self::calibration::CalibrationExt;
use self::conflict::ConflictResolutionExt;
use self::strategy::DeduplicationStrategyExt;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::deduplication";

/// Combined calibration, deduplication, conflict resolution, and
/// threshold filtering operation.
///
/// Created from the [`Deduplication`] graph node configuration.
pub struct DeduplicationOp {
    grouping: GroupingCriteria,
    strategy: DeduplicationStrategy,
    calibration: CalibrationMap,
    confidence_threshold: Option<f64>,
    conflict_resolution: ConflictResolution,
}

impl DeduplicationOp {
    /// Create from a [`Deduplication`] graph node config.
    pub fn new(cfg: &Deduplication) -> Self {
        tracing::debug!(
            target: TARGET,
            grouping = ?cfg.grouping,
            strategy = ?cfg.strategy,
            calibration_methods = cfg.calibration.len(),
            confidence_threshold = ?cfg.confidence_threshold,
            conflict_resolution = ?cfg.conflict_resolution,
            "creating deduplication operation",
        );
        Self {
            grouping: cfg.grouping,
            strategy: cfg.strategy.clone(),
            calibration: cfg.calibration.clone(),
            confidence_threshold: cfg.confidence_threshold,
            conflict_resolution: cfg.conflict_resolution,
        }
    }

    /// Run the full deduplication pipeline.
    pub(crate) fn deduplicate(&self, mut entities: Entities) -> Entities {
        if entities.is_empty() {
            return entities;
        }

        let before = entities.len();

        // Step 1: calibrate raw confidence scores.
        self.calibration.calibrate(&mut entities);

        // Step 2: group + fuse.
        let mut result = self.strategy.fuse(entities, self.grouping);

        // Step 3: resolve cross-kind span conflicts.
        result = self.conflict_resolution.resolve(result);

        // Step 4: filter by confidence threshold.
        let dropped = if let Some(threshold) = self.confidence_threshold {
            let before_filter = result.len();
            result = result.above_confidence(threshold);
            before_filter - result.len()
        } else {
            0
        };

        tracing::info!(
            target: TARGET,
            before,
            after = result.len(),
            reduced = before.saturating_sub(result.len()),
            dropped,
            "deduplication complete",
        );

        result
    }
}

impl Operation for DeduplicationOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if !envelope.audit.entities.is_empty() {
            tracing::debug!(
                target: TARGET,
                entities = envelope.audit.entities.len(),
                "running deduplication",
            );
            let entities = std::mem::take(&mut envelope.audit.entities);
            envelope.audit.entities = self.deduplicate(entities);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nvisy_ontology::entity::{
        Entity, EntityCategory, EntityKind, ExtractionMethod, Location, ModelKind,
        RecognitionMethod, RecognitionMethodKind, RefinementMethod, TextLocation,
    };
    use nvisy_ontology::workflow::DeduplicationStrategy::*;

    use super::*;

    /// Build a text entity at the given byte offsets for testing.
    fn text_entity(
        value: &str,
        method: RecognitionMethod,
        confidence: f64,
        start: usize,
        end: usize,
    ) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![method])
            .with_confidence(confidence)
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

    #[test]
    fn strict_groups_exact_overlap() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity("John", RecognitionMethod::regex("test"), 0.9, 0, 4),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn strict_preserves_non_overlapping() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity("John", RecognitionMethod::regex("test"), 0.9, 10, 14),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn normalized_groups_case_insensitive() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "john",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                0,
                4,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Normalized);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn narrowing_groups_substring_with_overlap() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John Smith",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                0,
                10,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Narrowing);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text_value().unwrap(), "John Smith");
    }

    #[test]
    fn narrowing_preserves_non_overlapping_substrings() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John Smith",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                100,
                110,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Narrowing);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn widening_groups_across_locations() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John Smith",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                100,
                110,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Widening);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text_value().unwrap(), "John Smith");
    }

    #[test]
    fn max_confidence_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.7, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.85,
                0,
                4,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn noisy_or_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.7, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.8,
                0,
                4,
            ),
        ]
        .into();
        let result = NoisyOr.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.94).abs() < 0.001);
    }

    #[test]
    fn weighted_average_strategy() {
        let mut weights = HashMap::new();
        weights.insert(RecognitionMethodKind::Regex, 1.0);
        weights.insert(RecognitionMethodKind::Ner, 2.0);

        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.6, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                0,
                4,
            ),
        ]
        .into();
        let result = WeightedAverage { weights }.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn calibration_scales_confidence() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Regex, 0.5);

        let mut entities: Entities = vec![text_entity(
            "John",
            RecognitionMethod::regex("test"),
            0.8,
            0,
            4,
        )]
        .into();
        calibration.calibrate(&mut entities);
        assert!((entities[0].confidence - 0.4).abs() < f64::EPSILON);
        assert!(
            entities[0]
                .refinement_methods
                .contains(&RefinementMethod::ConfidenceCalibration)
        );
    }

    #[test]
    fn calibration_clamps_to_one() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Regex, 2.0);

        let mut entities: Entities = vec![text_entity(
            "John",
            RecognitionMethod::regex("test"),
            0.8,
            0,
            4,
        )]
        .into();
        calibration.calibrate(&mut entities);
        assert!((entities[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuse_picks_longest_value() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.9, 0, 4),
            text_entity(
                "John Smith",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.7,
                0,
                10,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Widening);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text_value().unwrap(), "John Smith");
    }

    #[test]
    fn fuse_merges_extraction_methods() {
        let mut e1 = text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4);
        e1.extraction_methods = vec![ExtractionMethod::DocumentParsing];
        let mut e2 = text_entity(
            "John",
            RecognitionMethod::ner("test", ModelKind::SelfHosted),
            0.9,
            0,
            4,
        );
        e2.extraction_methods = vec![ExtractionMethod::OpticalCharacterRecognition];

        let entities: Entities = vec![e1, e2].into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result[0].extraction_methods.len(), 2);
    }

    #[test]
    fn fuse_fills_missing_language() {
        let mut e1 = text_entity("John", RecognitionMethod::regex("test"), 0.9, 0, 4);
        let mut e2 = text_entity(
            "John",
            RecognitionMethod::ner("test", ModelKind::SelfHosted),
            0.7,
            0,
            4,
        );
        e1.language = None;
        e2.language = Some("en".into());

        let entities: Entities = vec![e1, e2].into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result[0].language.as_deref(), Some("en"));
    }

    #[test]
    fn same_detector_duplicates_tagged_as_deduplication() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity("John", RecognitionMethod::regex("other"), 0.9, 0, 4),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .refinement_methods
                .contains(&RefinementMethod::Deduplication)
        );
    }

    #[test]
    fn different_detector_tagged_as_ensemble_fusion() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                0,
                4,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::default());
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .refinement_methods
                .contains(&RefinementMethod::EnsembleFusion)
        );
    }

    #[test]
    fn confidence_threshold_filters() {
        let cfg = Deduplication {
            confidence_threshold: Some(0.85),
            ..Default::default()
        };
        let op = DeduplicationOp::new(&cfg);
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.9, 0, 4),
            text_entity("Jane", RecognitionMethod::regex("test"), 0.5, 10, 14),
        ]
        .into();
        let result = op.deduplicate(entities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text_value().unwrap(), "John");
    }

    #[test]
    fn full_pipeline() {
        let cfg = Deduplication {
            strategy: DeduplicationStrategy::MaxConfidence,
            ..Default::default()
        };
        let op = DeduplicationOp::new(&cfg);
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.7, 0, 4),
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.85,
                0,
                4,
            ),
        ]
        .into();

        let result = op.deduplicate(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_input() {
        let cfg = Deduplication::default();
        let op = DeduplicationOp::new(&cfg);
        let result = op.deduplicate(Entities::new());
        assert!(result.is_empty());
    }

    #[test]
    fn calibration_uses_max_across_multiple_methods() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Regex, 0.5);
        calibration.insert(RecognitionMethodKind::Ner, 0.8);

        let entity = Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![
                RecognitionMethod::regex("test"),
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
            ])
            .with_confidence(1.0)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_text("John")
                    .with_start_offset(0usize)
                    .with_end_offset(4usize)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();

        let mut entities: Entities = vec![entity].into();
        calibration.calibrate(&mut entities);
        // max(0.5, 0.8) = 0.8; 1.0 * 0.8 = 0.8
        assert!((entities[0].confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn single_entity_passes_through_unchanged() {
        let entity = text_entity("John", RecognitionMethod::regex("test"), 0.75, 10, 14);
        let entities: Entities = vec![entity.clone()].into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text_value().unwrap(), "John");
        assert!((result[0].confidence - 0.75).abs() < f64::EPSILON);
        assert_eq!(
            result[0].recognition_methods,
            vec![RecognitionMethod::regex("test")]
        );
    }

    #[test]
    fn transitive_overlap_groups_correctly() {
        // A overlaps B, B overlaps C, but A does not overlap C.
        // All three should end up in the same group.
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.7, 0, 6),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.8,
                4,
                10,
            ),
            text_entity(
                "John",
                RecognitionMethod::ner("test", ModelKind::SelfHosted),
                0.9,
                8,
                14,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
    }
}
