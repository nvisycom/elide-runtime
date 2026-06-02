//! Entity deduplication: composable layers run through a
//! [`LayerPipeline`].
//!
//! Public surface is the [`Layer`] trait, the four built-in layer
//! types ([`CalibrateLayer`], [`FilterLayer`], [`FuseLayer`],
//! [`ResolveConflictsLayer`]), the [`LayerPipeline`] orchestrator,
//! and the per-layer config types ([`CalibrationMap`], [`FilterParams`],
//! [`DeduplicationStrategy`], [`GroupingCriteria`],
//! [`ConflictResolution`], [`SpanSize`]).
//!
//! The phase orchestrator that drives this per [`DocumentTree`] node
//! lives in [`DeduplicationPhase`]. It calls
//! [`LayerPipeline::from_params`] to assemble the canonical four-step
//! recipe, then runs the pipeline against each node's entities.
//!
//! # Canonical recipe
//!
//! 1. **Calibrate** raw confidence scores per-recognizer.
//! 2. **Filter** by allowed kinds + confidence floor.
//! 3. **Fuse** co-referent entities into one (group + combine).
//! 4. **Resolve conflicts** between different kinds on the same span.
//!
//! Operators can swap steps, drop steps, or insert their own custom
//! [`Layer`] impls by building the pipeline manually with
//! [`LayerPipeline::new`] + [`LayerPipeline::with_layer`].
//!
//! [`DocumentTree`]: crate::core::DocumentTree
//! [`DeduplicationPhase`]: crate::pipeline::DeduplicationPhase

mod calibrate;
mod filter;
mod fuse;
mod layer;
mod pipeline;
mod resolve;
mod span_size;

pub use self::calibrate::{CalibrateLayer, CalibrationMap};
pub use self::filter::{FilterLayer, FilterParams};
pub use self::fuse::{DeduplicationStrategy, FuseLayer, GroupingCriteria};
pub use self::layer::{Layer, LayerContext};
pub use self::pipeline::LayerPipeline;
pub use self::resolve::{ConflictResolution, ResolveConflictsLayer};
pub use self::span_size::SpanSize;

#[cfg(test)]
pub(crate) fn test_resolver<M: nvisy_ontology::modality::Modality>()
-> Box<dyn crate::core::ValueAt<M>> {
    use async_trait::async_trait;

    struct Noop<M>(std::marker::PhantomData<M>);

    #[async_trait]
    impl<M: nvisy_ontology::modality::Modality> crate::core::ValueAt<M> for Noop<M> {
        async fn value_at(&self, _location: &M) -> Option<String> {
            None
        }
    }

    Box::new(Noop::<M>(std::marker::PhantomData))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_ontology::document::Document;
    use nvisy_ontology::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
    use nvisy_ontology::modality::{Text, TextExtraction, TextMetadata};
    use nvisy_ontology::primitive::{Confidence, ConfidenceThreshold};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::*;
    use crate::core::{DocumentView, SharedHandle, ValueAt};
    use crate::pipeline::DeduplicationParams;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    const TEST_TEXT: &str = "John Smith";

    /// Owned per-test components used to drive the dedup pipeline.
    struct Fixture {
        handle: SharedHandle,
        doc: Document<Text>,
    }

    async fn test_fixture(text: &str) -> Fixture {
        let handle: SharedHandle = Arc::new(Mutex::new(
            nvisy_formats::test_utils::decode_text(text)
                .await
                .expect("decode text"),
        ));
        let source = handle.lock().await.source();
        let doc = Document::<Text>::new(
            TextMetadata {
                extraction: TextExtraction::Native,
                languages: Vec::new(),
            },
            source,
        );
        Fixture { handle, doc }
    }

    #[tokio::test]
    async fn confidence_threshold_filters() {
        let fix = test_fixture("John......Jane").await;
        let view = DocumentView::new(&fix.doc, &fix.handle);
        let filter = FilterParams {
            confidence_threshold: Some(ConfidenceThreshold::clamped(0.85)),
            ..Default::default()
        };
        let entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(10, 14)
                .with_confidence(conf(0.5))
                .test_build(),
        ];
        let pipeline: LayerPipeline<Text, _> =
            LayerPipeline::from_params(&DeduplicationParams::default(), filter);
        let ctx = LayerContext::new(&view).with_correlation_id(Uuid::nil());
        let result = pipeline.run(entities, &ctx).await;
        assert_eq!(result.len(), 1);
        let value = view.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
    }

    #[tokio::test]
    async fn full_pipeline() {
        let fix = test_fixture(TEST_TEXT).await;
        let view = DocumentView::new(&fix.doc, &fix.handle);
        let entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.7))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_trail(vec![TrailStep::recognition(
                    "ner",
                    conf(0.85),
                    TrailProvenance::Model(ModelProvenance::new("test")),
                    "",
                )])
                .with_confidence(conf(0.85))
                .test_build(),
        ];
        let pipeline: LayerPipeline<Text, _> =
            LayerPipeline::from_params(&DeduplicationParams::default(), FilterParams::default());
        let ctx = LayerContext::new(&view).with_correlation_id(Uuid::nil());
        let result = pipeline.run(entities, &ctx).await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence.get() - 0.85).abs() < f64::EPSILON);
    }
}
