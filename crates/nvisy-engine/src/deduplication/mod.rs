//! Entity deduplication phase.
//!
//! Runs after detection. Combines multiple detection passes into a
//! single, deduplicated set of entities with combined confidence
//! scores.
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

mod calibrate;
mod filter;
mod fuse;
mod params;
mod resolve;
mod span_size;

use std::mem;

use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::modality::AnyModality;

use self::calibrate::Calibrate;
pub use self::calibrate::CalibrationMap;
use self::filter::Filter;
pub use self::filter::FilterParams;
use self::fuse::Fuse;
pub use self::fuse::{DeduplicationStrategy, GroupingCriteria};
pub use self::params::DeduplicationParams;
pub use self::resolve::ConflictResolution;
use self::resolve::ResolveConflicts;
use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::deduplication";

/// Combined calibration, deduplication, conflict resolution, and
/// threshold filtering operation.
///
/// Created from the [`DeduplicationParams`] workflow configuration.
pub struct Deduplicator {
    grouping: GroupingCriteria,
    strategy: DeduplicationStrategy,
    calibration: CalibrationMap,
    conflict_resolution: ConflictResolution,
}

impl Deduplicator {
    /// Create from a [`DeduplicationParams`] workflow config.
    pub fn new(cfg: &DeduplicationParams) -> Self {
        tracing::debug!(
            target: TARGET,
            grouping = ?cfg.grouping,
            strategy = ?cfg.strategy,
            calibration_methods = cfg.calibration.len(),
            conflict_resolution = ?cfg.conflict_resolution,
            "creating deduplication operation",
        );
        Self {
            grouping: cfg.grouping,
            strategy: cfg.strategy.clone(),
            calibration: cfg.calibration.clone(),
            conflict_resolution: cfg.conflict_resolution,
        }
    }

    /// Run the full deduplication pipeline.
    pub(crate) async fn deduplicate(
        &self,
        mut entities: Entities<AnyModality>,
        envelope: &DocumentEnvelope,
        params: &FilterParams,
    ) -> Entities<AnyModality> {
        if entities.is_empty() {
            return entities;
        }

        let before = entities.len();

        // Step 1: calibrate raw confidence scores.
        entities.calibrate(&self.calibration);

        // Step 2: filter by operator-supplied (or engine-default)
        // params on the calibrated score.
        let dropped = entities.filter(params);

        // Step 3: group + fuse.
        entities.fuse(&self.strategy, self.grouping, envelope).await;

        // Step 4: resolve cross-kind span conflicts.
        let _conflict_dropped = entities.resolve_conflicts(&self.conflict_resolution);

        tracing::info!(
            target: TARGET,
            before,
            after = entities.len(),
            reduced = before.saturating_sub(entities.len()),
            dropped = dropped.len(),
            "deduplication complete",
        );

        entities
    }

    /// Execute deduplication against the envelope's entities.
    pub async fn execute(
        &self,
        envelope: &mut DocumentEnvelope,
        params: &FilterParams,
    ) -> Result<()> {
        if !envelope.audit.entities.is_empty() {
            tracing::debug!(
                target: TARGET,
                entities = envelope.audit.entities.len(),
                "running deduplication",
            );
            let entities = mem::take(&mut envelope.audit.entities);
            envelope.audit.entities = self.deduplicate(entities, envelope, params).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, ModelKind, RecognitionMethod};
    use nvisy_ontology::primitive::Confidence;

    use super::*;
    use crate::envelope::Document;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    const TEST_TEXT: &str = "John Smith";

    #[tokio::test]
    async fn confidence_threshold_filters() {
        let doc = Document::from_text("John......Jane").await;
        let op = Deduplicator::new(&DeduplicationParams::default());
        let entities: Entities = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(10, 14)
                .with_confidence(conf(0.5))
                .test_build(),
        ]
        .into();
        let params = FilterParams {
            confidence_threshold: Some(0.85),
            ..Default::default()
        };
        let result = op.deduplicate(entities, &doc, &params).await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
    }

    #[tokio::test]
    async fn full_pipeline() {
        let doc = Document::from_text(TEST_TEXT).await;
        let op = Deduplicator::new(&DeduplicationParams::default());
        let entities: Entities = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.7))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(conf(0.85))
                .test_build(),
        ]
        .into();
        let result = op
            .deduplicate(entities, &doc, &FilterParams::default())
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence.get() - 0.85).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn empty_input() {
        let doc = Document::from_text("").await;
        let op = Deduplicator::new(&DeduplicationParams::default());
        let result = op
            .deduplicate(Entities::new(), &doc, &FilterParams::default())
            .await;
        assert!(result.is_empty());
    }
}
