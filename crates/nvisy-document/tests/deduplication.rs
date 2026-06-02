//! Deduplication layer integration tests.
//!
//! Exercises the toolkit's `LayerPipeline` against real `Document<Text>`
//! values constructed from in-memory text. These tests live here
//! rather than in `nvisy-toolkit` because they cross the toolkit ↔
//! document boundary (they need `Document`, `TextMetadata`, and the
//! formats `decode_text` helper).

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::ValueAt;
use nvisy_core::content::ContentMetadata;
use nvisy_core::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep, TrailStepKind};
use nvisy_core::modality::Text;
use nvisy_core::primitive::{Confidence, ConfidenceThreshold};
use nvisy_document::core::{DocumentView, SharedData, SharedHandle};
use nvisy_document::document::Document;
use nvisy_document::modality::{TextExtraction, TextMetadata};
use nvisy_document::phases::ingestion::registry::Registry;
use nvisy_formats::test_utils::decode_text;
use nvisy_toolkit::deduplication::{
    DeduplicationParams, DeduplicationStrategy, FilterParams, FuseLayer, GroupingCriteria, Layer,
    LayerContext, LayerPipeline,
};
use tokio::sync::Mutex;
use uuid::Uuid;

fn conf(v: f64) -> Confidence {
    Confidence::new(v).expect("confidence in [0,1]")
}

fn ner_step(confidence: Confidence) -> TrailStep {
    TrailStep::recognition(
        "ner",
        confidence,
        TrailProvenance::Model(ModelProvenance::new("test")),
        "",
    )
}

const TEXT: &str = "John Smith";

struct Fixture {
    handle: SharedHandle,
    doc: Document<Text>,
    _metadata: ContentMetadata,
    _shared: Arc<SharedData>,
}

async fn test_fixture(text: &str) -> Fixture {
    let handle: SharedHandle = Arc::new(Mutex::new(decode_text(text).await.expect("decode text")));
    let source = handle.lock().await.source();
    let doc = Document::<Text>::new(
        TextMetadata {
            extraction: TextExtraction::Native,
            languages: Vec::new(),
        },
        source,
    );
    let metadata = ContentMetadata::new().with_content_type("text/plain");
    let registry =
        Registry::open(tempfile::tempdir().expect("tempdir").path()).expect("open registry");
    let shared = SharedData::new(Uuid::nil(), Uuid::nil(), registry);
    Fixture {
        handle,
        doc,
        _metadata: metadata,
        _shared: shared,
    }
}

async fn fuse_with(
    strategy: DeduplicationStrategy,
    criteria: GroupingCriteria,
    view: &DocumentView<'_, Text>,
    entities: &mut Vec<Entity<Text>>,
) {
    let ctx = LayerContext::new(view);
    let layer = FuseLayer::new(strategy, criteria);
    let dropped = layer.apply(entities, &ctx).await;
    assert!(dropped.is_empty(), "fuse never drops");
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
    let fix = test_fixture(TEXT).await;
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

#[tokio::test]
async fn strict_grouping_fuses_identical_spans_with_max_confidence() {
    let fix = test_fixture(TEXT).await;
    let view = DocumentView::new(&fix.doc, &fix.handle);
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4)
            .with_confidence(conf(0.8))
            .test_build(),
        Entity::test_builder(0, 4).test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::MaxConfidence,
        GroupingCriteria::Strict,
        &view,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    assert!((entities[0].confidence.get() - 0.9).abs() < f64::EPSILON);
}

#[tokio::test]
async fn narrowing_groups_substring_with_overlap() {
    let fix = test_fixture(TEXT).await;
    let view = DocumentView::new(&fix.doc, &fix.handle);
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4)
            .with_confidence(conf(0.8))
            .test_build(),
        Entity::test_builder(0, 10)
            .with_trail(vec![ner_step(conf(0.9))])
            .test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::MaxConfidence,
        GroupingCriteria::Narrowing,
        &view,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    let value = view.value_at(&entities[0].location).await;
    assert_eq!(value.as_deref(), Some("John Smith"));
}

#[tokio::test]
async fn widening_groups_across_non_overlapping_locations() {
    let text = format!("{:<100}John Smith", TEXT);
    let fix = test_fixture(&text).await;
    let view = DocumentView::new(&fix.doc, &fix.handle);
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4).test_build(),
        Entity::test_builder(100, 110)
            .with_trail(vec![ner_step(conf(0.9))])
            .test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::MaxConfidence,
        GroupingCriteria::Widening,
        &view,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
}

#[tokio::test]
async fn noisy_or_strategy() {
    let fix = test_fixture(TEXT).await;
    let view = DocumentView::new(&fix.doc, &fix.handle);
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4)
            .with_confidence(conf(0.7))
            .test_build(),
        Entity::test_builder(0, 4)
            .with_trail(vec![ner_step(conf(0.9))])
            .with_confidence(conf(0.8))
            .test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::NoisyOr,
        GroupingCriteria::default(),
        &view,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    // 1 - (1 - 0.7)(1 - 0.8) = 1 - 0.06 = 0.94
    assert!((entities[0].confidence.get() - 0.94).abs() < 0.001);
}

#[tokio::test]
async fn weighted_average_strategy() {
    let fix = test_fixture(TEXT).await;
    let view = DocumentView::new(&fix.doc, &fix.handle);
    let mut weights = HashMap::new();
    weights.insert("pattern".to_string(), 1.0);
    weights.insert("ner".to_string(), 2.0);

    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4)
            .with_confidence(conf(0.6))
            .test_build(),
        Entity::test_builder(0, 4)
            .with_trail(vec![ner_step(conf(0.9))])
            .test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::WeightedAverage { weights },
        GroupingCriteria::default(),
        &view,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    // (0.6 * 1.0 + 0.9 * 2.0) / 3.0 = 0.8
    assert!((entities[0].confidence.get() - 0.8).abs() < 0.001);
}

#[tokio::test]
async fn different_detector_tagged_as_ensemble_fusion() {
    let fix = test_fixture(TEXT).await;
    let view = DocumentView::new(&fix.doc, &fix.handle);
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4)
            .with_confidence(conf(0.8))
            .test_build(),
        Entity::test_builder(0, 4)
            .with_trail(vec![ner_step(conf(0.9))])
            .test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::MaxConfidence,
        GroupingCriteria::default(),
        &view,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    assert!(
        entities[0]
            .trail
            .iter()
            .any(|s| matches!(s.kind, TrailStepKind::Fusion) && s.reason.contains("ensemble"))
    );
}
