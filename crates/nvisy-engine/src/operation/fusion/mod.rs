//! Entity fusion operation.
//!
//! Runs at **phase 3**, after detection. Combines multiple detection
//! passes into a single, deduplicated set of entities with combined
//! confidence scores.
//!
//! The fusion pipeline:
//!
//! 1. **Calibrate**: scale raw confidence scores using per-method
//!    multipliers from [`CalibrationMap`].
//! 2. **Group + fuse**: partition entities by kind, value, and
//!    location overlap (per [`GroupingCriteria`]), then combine each
//!    group into one entity using the [`FusionStrategy`].
//!
//! Deduplication is implicit: entities that land in the same group
//! are always merged, so exact duplicates are naturally eliminated.

mod calibration;
mod grouping;
mod strategy;

use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::workflow::{CalibrationMap, Fusion, FusionStrategy, GroupingCriteria};

use self::calibration::calibrate;
use self::strategy::FusionStrategyExt;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::fusion";

/// Combined calibration and ensemble fusion operation.
///
/// Created from the [`Fusion`] graph node configuration via
/// [`FusionOp::new`]. The heavy lifting is delegated to the
/// [`calibration`], [`grouping`], and [`strategy`] sub-modules.
pub struct FusionOp {
    grouping: GroupingCriteria,
    strategy: FusionStrategy,
    calibration: CalibrationMap,
}

impl FusionOp {
    /// Create from a [`Fusion`] graph node config.
    pub fn new(cfg: &Fusion) -> Self {
        tracing::debug!(
            target: TARGET,
            grouping = ?cfg.grouping,
            strategy = ?cfg.strategy,
            calibration_methods = cfg.calibration.len(),
            "creating fusion operation",
        );
        Self {
            grouping: cfg.grouping,
            strategy: cfg.strategy.clone(),
            calibration: cfg.calibration.clone(),
        }
    }

    /// Run the full fusion pipeline: calibrate, group, fuse.
    pub(crate) fn fuse(&self, mut entities: Entities) -> Entities {
        if entities.is_empty() {
            return entities;
        }

        let before = entities.len();

        // Phase 1: calibrate raw confidence scores.
        if !self.calibration.is_empty() {
            calibrate(&mut entities, &self.calibration);
        }

        // Phase 2: group + fuse.
        let result = self.strategy.fuse(entities, self.grouping);

        tracing::info!(
            target: TARGET,
            before,
            after = result.len(),
            reduced = before.saturating_sub(result.len()),
            "fusion complete",
        );

        result
    }
}

impl Operation for FusionOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if !envelope.audit.entities.is_empty() {
            tracing::debug!(
                target: TARGET,
                entities = envelope.audit.entities.len(),
                "running fusion",
            );
            let entities = std::mem::take(&mut envelope.audit.entities);
            envelope.audit.entities = self.fuse(entities);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nvisy_ontology::entity::{
        Entity, EntityCategory, EntityKind, ExtractionMethod, Location, ModelInfo, ModelKind,
        RecognitionMethod, RecognitionMethodKind, TextLocation,
    };
    use nvisy_ontology::workflow::FusionStrategy::*;

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
            .with_value(value)
            .with_recognition_methods(vec![method])
            .with_confidence(confidence)
            .with_location(Location::from(TextLocation {
                start_offset: start,
                end_offset: end,
                ..Default::default()
            }))
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
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
                0.9,
                0,
                10,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Narrowing);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John Smith");
    }

    #[test]
    fn narrowing_preserves_non_overlapping_substrings() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John Smith",
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
                0.9,
                100,
                110,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Widening);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John Smith");
    }

    #[test]
    fn max_confidence_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.7, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
        calibrate(&mut entities, &calibration);
        assert!((entities[0].confidence - 0.4).abs() < f64::EPSILON);
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
        calibrate(&mut entities, &calibration);
        assert!((entities[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuse_picks_longest_value() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.9, 0, 4),
            text_entity(
                "John Smith",
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
                0.7,
                0,
                10,
            ),
        ]
        .into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Widening);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John Smith");
    }

    #[test]
    fn fuse_merges_extraction_methods() {
        let mut e1 = text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4);
        e1.extraction_methods = vec![ExtractionMethod::DocumentParsing];
        let mut e2 = text_entity(
            "John",
            RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
            RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
    fn full_pipeline() {
        let cfg = Fusion {
            strategy: FusionStrategy::MaxConfidence,
            ..Default::default()
        };
        let fusion = FusionOp::new(&cfg);
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::regex("test"), 0.7, 0, 4),
            text_entity("John", RecognitionMethod::regex("test"), 0.8, 0, 4),
            text_entity(
                "John",
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
                0.85,
                0,
                4,
            ),
        ]
        .into();

        let result = fusion.fuse(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_input() {
        let cfg = Fusion::default();
        let fusion = FusionOp::new(&cfg);
        let result = fusion.fuse(Entities::new());
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
            .with_value("John")
            .with_recognition_methods(vec![
                RecognitionMethod::regex("test"),
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
            ])
            .with_confidence(1.0)
            .build()
            .unwrap();

        let mut entities: Entities = vec![entity].into();
        calibrate(&mut entities, &calibration);
        // max(0.5, 0.8) = 0.8; 1.0 * 0.8 = 0.8
        assert!((entities[0].confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn single_entity_passes_through_unchanged() {
        let entity = text_entity("John", RecognitionMethod::regex("test"), 0.75, 10, 14);
        let entities: Entities = vec![entity.clone()].into();
        let result = MaxConfidence.fuse(entities, GroupingCriteria::Strict);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, "John");
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
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
                0.8,
                4,
                10,
            ),
            text_entity(
                "John",
                RecognitionMethod::ner(ModelInfo::new("test", ModelKind::SelfHosted)),
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
