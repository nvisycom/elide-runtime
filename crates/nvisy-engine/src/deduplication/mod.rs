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
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::provenance::EntityRecord;

use self::calibrate::Calibrate;
pub use self::calibrate::CalibrationMap;
use self::filter::Filter;
pub use self::filter::FilterParams;
use self::fuse::Fuse;
pub use self::fuse::{DeduplicationStrategy, GroupingCriteria};
pub use self::params::DeduplicationParams;
pub use self::resolve::ConflictResolution;
use self::resolve::ResolveConflicts;
pub use self::span_size::SpanSize;
use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;

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
    pub(crate) async fn deduplicate<M>(
        &self,
        mut entities: Vec<Entity<M>>,
        envelope: &DocumentEnvelope<M>,
        params: &FilterParams,
    ) -> Vec<Entity<M>>
    where
        M: Modality + Overlap + SpanSize,
        DocumentEnvelope<M>: ValueAt<M>,
    {
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
    pub async fn execute<M>(
        &self,
        envelope: &mut DocumentEnvelope<M>,
        params: &FilterParams,
    ) -> Result<()>
    where
        M: Modality + Overlap + SpanSize,
        DocumentEnvelope<M>: ValueAt<M>,
    {
        if !envelope.document.audit.records.is_empty() {
            tracing::debug!(
                target: TARGET,
                entities = envelope.document.audit.records.len(),
                "running deduplication",
            );
            // Dedup runs before redaction evaluation, so every
            // record's `audit` is still None; we can pull entities
            // out, dedup, and rewrap without losing audit state.
            let records = mem::take(&mut envelope.document.audit.records);
            let entities: Vec<Entity<M>> = records.into_iter().map(|r| r.entity).collect();
            let deduped = self.deduplicate(entities, envelope, params).await;
            envelope.document.audit.records = deduped.into_iter().map(EntityRecord::new).collect();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_core::content::ContentMetadata;
    use nvisy_ontology::entity::{Entity, ModelKind, RecognitionMethod};
    use nvisy_ontology::modality::{Text, TextExtraction, TextMetadata};
    use nvisy_ontology::primitive::{Confidence, ConfidenceThreshold};
    use tokio::sync::Mutex;

    use super::*;
    use crate::envelope::SharedData;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    const TEST_TEXT: &str = "John Smith";

    async fn test_envelope(text: &str) -> DocumentEnvelope<Text> {
        let registry = crate::ingestion::registry::Registry::open(
            tempfile::tempdir().expect("tempdir").path(),
        )
        .expect("open registry");
        let shared = SharedData::new(uuid::Uuid::nil(), uuid::Uuid::nil(), registry);
        let handle = nvisy_formats::test_utils::decode_text(text)
            .await
            .expect("decode text");
        DocumentEnvelope::<Text>::new(
            Arc::new(Mutex::new(handle)),
            ContentMetadata::new().with_content_type("text/plain"),
            TextMetadata {
                extraction: TextExtraction::Native,
                languages: Vec::new(),
            },
            shared,
        )
        .await
    }

    #[tokio::test]
    async fn confidence_threshold_filters() {
        let doc = test_envelope("John......Jane").await;
        let op = Deduplicator::new(&DeduplicationParams::default());
        let entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(10, 14)
                .with_confidence(conf(0.5))
                .test_build(),
        ];
        let params = FilterParams {
            confidence_threshold: Some(ConfidenceThreshold::clamped(0.85)),
            ..Default::default()
        };
        let result = op.deduplicate(entities, &doc, &params).await;
        assert_eq!(result.len(), 1);
        let value = doc.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
    }

    #[tokio::test]
    async fn full_pipeline() {
        let doc = test_envelope(TEST_TEXT).await;
        let op = Deduplicator::new(&DeduplicationParams::default());
        let entities: Vec<Entity<Text>> = vec![
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
        ];
        let result = op
            .deduplicate(entities, &doc, &FilterParams::default())
            .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence.get() - 0.85).abs() < f64::EPSILON);
    }
}
