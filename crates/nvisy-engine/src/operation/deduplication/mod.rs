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
    pub(crate) async fn deduplicate(
        &self,
        mut entities: Entities,
        document: &crate::operation::Document,
    ) -> Entities {
        if entities.is_empty() {
            return entities;
        }

        let before = entities.len();

        // Step 1: calibrate raw confidence scores.
        self.calibration.calibrate(&mut entities);

        // Step 2: group + fuse.
        let mut result = self.strategy.fuse(entities, self.grouping, document).await;

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
            envelope.audit.entities = self.deduplicate(entities, &envelope.document).await;
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
    use crate::operation::Document;

    /// The test document that entities reference by byte offset.
    /// "John Smith" at 0..10, then padding, then "Jane" at 100..104.
    const TEST_TEXT: &str = "John Smith";

    #[tokio::test]
    async fn strict_groups_exact_overlap() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(0, 4).test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Strict, &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn strict_preserves_non_overlapping() {
        let doc = Document::from_text("John......John").await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(10, 14).test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Strict, &doc)
            .await;
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn normalized_groups_case_insensitive() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Normalized, &doc)
            .await;
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn narrowing_groups_substring_with_overlap() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(0, 10)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Narrowing, &doc)
            .await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John Smith"));
    }

    #[tokio::test]
    async fn narrowing_preserves_non_overlapping_substrings() {
        // Pad to 110 chars so offset 100..110 is valid.
        let text = format!("{:<100}John Smith", TEST_TEXT);
        let doc = Document::from_text(&text).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(100, 110)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Narrowing, &doc)
            .await;
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn widening_groups_across_locations() {
        let text = format!("{:<100}John Smith", TEST_TEXT);
        let doc = Document::from_text(&text).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(100, 110)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Widening, &doc)
            .await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John Smith"));
    }

    #[tokio::test]
    async fn max_confidence_strategy() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.7).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(0.85)
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn noisy_or_strategy() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.7).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(0.8)
                .test_build(),
        ]
        .into();
        let result = NoisyOr
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.94).abs() < 0.001);
    }

    #[tokio::test]
    async fn weighted_average_strategy() {
        let doc = Document::from_text(TEST_TEXT).await;
        let mut weights = HashMap::new();
        weights.insert(RecognitionMethodKind::Regex, 1.0);
        weights.insert(RecognitionMethodKind::Ner, 2.0);

        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.6).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = (WeightedAverage { weights })
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn calibration_scales_confidence() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Regex, 0.5);

        let mut entities: Entities =
            vec![Entity::test_builder(0, 4).with_confidence(0.8).test_build()].into();
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

        let mut entities: Entities =
            vec![Entity::test_builder(0, 4).with_confidence(0.8).test_build()].into();
        calibration.calibrate(&mut entities);
        assert!((entities[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn fuse_picks_longest_value() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(0, 10)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(0.7)
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Widening, &doc)
            .await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John Smith"));
    }

    #[tokio::test]
    async fn fuse_merges_extraction_methods() {
        let doc = Document::from_text(TEST_TEXT).await;
        let mut e1 = Entity::test_builder(0, 4).with_confidence(0.8).test_build();
        e1.extraction_methods = vec![ExtractionMethod::DocumentParsing];
        let mut e2 = Entity::test_builder(0, 4)
            .with_recognition_methods(vec![RecognitionMethod::ner("test", ModelKind::SelfHosted)])
            .test_build();
        e2.extraction_methods = vec![ExtractionMethod::OpticalCharacterRecognition];

        let entities: Entities = vec![e1, e2].into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result[0].extraction_methods.len(), 2);
    }

    #[tokio::test]
    async fn fuse_fills_missing_language() {
        let doc = Document::from_text(TEST_TEXT).await;
        let mut e1 = Entity::test_builder(0, 4).test_build();
        let mut e2 = Entity::test_builder(0, 4)
            .with_recognition_methods(vec![RecognitionMethod::ner("test", ModelKind::SelfHosted)])
            .with_confidence(0.7)
            .test_build();
        e1.language = None;
        e2.language = Some("en".parse().unwrap());

        let entities: Entities = vec![e1, e2].into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result[0].language.as_ref().map(|t| t.as_str()), Some("en"));
    }

    #[tokio::test]
    async fn same_detector_duplicates_tagged_as_deduplication() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::regex("other")])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .refinement_methods
                .contains(&RefinementMethod::Deduplication)
        );
    }

    #[tokio::test]
    async fn different_detector_tagged_as_ensemble_fusion() {
        let doc = Document::from_text(TEST_TEXT).await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::default(), &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .refinement_methods
                .contains(&RefinementMethod::EnsembleFusion)
        );
    }

    #[tokio::test]
    async fn confidence_threshold_filters() {
        let doc = Document::from_text("John......Jane").await;
        let cfg = Deduplication {
            confidence_threshold: Some(0.85),
            ..Default::default()
        };
        let op = DeduplicationOp::new(&cfg);
        let entities: Entities = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(10, 14)
                .with_confidence(0.5)
                .test_build(),
        ]
        .into();
        let result = op.deduplicate(entities, &doc).await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
    }

    #[tokio::test]
    async fn full_pipeline() {
        let doc = Document::from_text(TEST_TEXT).await;
        let cfg = Deduplication {
            strategy: DeduplicationStrategy::MaxConfidence,
            ..Default::default()
        };
        let op = DeduplicationOp::new(&cfg);
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.7).test_build(),
            Entity::test_builder(0, 4).with_confidence(0.8).test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(0.85)
                .test_build(),
        ]
        .into();

        let result = op.deduplicate(entities, &doc).await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn empty_input() {
        let doc = Document::from_text("").await;
        let cfg = Deduplication::default();
        let op = DeduplicationOp::new(&cfg);
        let result = op.deduplicate(Entities::new(), &doc).await;
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

    #[tokio::test]
    async fn single_entity_passes_through_unchanged() {
        let doc = Document::from_text("..........John").await;
        let entity = Entity::test_builder(10, 14)
            .with_confidence(0.75)
            .test_build();
        let entities: Entities = vec![entity.clone()].into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Strict, &doc)
            .await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
        assert!((result[0].confidence - 0.75).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn transitive_overlap_groups_correctly() {
        // Three entities with overlapping spans and substring-matching
        // values. A(0..4="John") overlaps B(2..10="hn Smit"), B overlaps
        // C(6..16="mith Jones"), but A does not overlap C. Using
        // Widening criteria, all three share substring relationships
        // and should end up in one group via transitive overlap.
        let doc = Document::from_text("John Smith Jones").await;
        let entities: Entities = vec![
            Entity::test_builder(0, 4).with_confidence(0.7).test_build(),
            Entity::test_builder(0, 10)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(0.8)
                .test_build(),
            Entity::test_builder(0, 16)
                .with_recognition_methods(vec![RecognitionMethod::ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .test_build(),
        ]
        .into();
        let result = MaxConfidence
            .fuse(entities, GroupingCriteria::Widening, &doc)
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
    }
}
