//! Deduplication layer integration tests.
//!
//! Exercises the toolkit's `LayerPipeline` against real
//! `DocumentTree<Text>` values constructed from in-memory text via
//! the codec registry. These tests live here rather than in
//! `nvisy-toolkit` because they cross the toolkit ↔ document
//! boundary (they need `DocumentTree`, `TextMetadata`, and a real
//! decoded codec handle).

use std::collections::HashMap;

use nvisy_codec::{CodecRegistry, UntypedDocumentHandle};
use nvisy_core::TextAt;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep, TrailStepKind};
use nvisy_core::modality::Text;
use nvisy_core::primitive::{Confidence, ConfidenceThreshold};
use nvisy_document::core::DocumentTree;
use nvisy_document::document::Document;
use nvisy_document::modality::{TextExtraction, TextMetadata};
use nvisy_formats::CodecRegistryExt;
use nvisy_toolkit::deduplication::{
    DeduplicationParams, DeduplicationStrategy, FilterParams, FuseLayer, GroupingCriteria, Layer,
    LayerContext, LayerPipeline,
};
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

async fn tree_from(text: &str) -> DocumentTree<Text> {
    let registry = CodecRegistry::builtins();
    let format = registry.by_extension("txt").expect("txt codec registered");
    let data = ContentData::new(ContentSource::new(), text.as_bytes().to_vec().into());
    let untyped = format.loader.decode(data).await.expect("decode");
    let handle = match untyped {
        UntypedDocumentHandle::Text(h) => h,
        _ => panic!("txt loader must produce Text"),
    };
    let source = *handle.handler().source();
    let doc = Document::<Text>::new(
        TextMetadata {
            extraction: TextExtraction::Native,
            languages: Vec::new(),
        },
        source,
    );
    let metadata = ContentMetadata::new().with_content_type("text/plain");
    DocumentTree::new(doc, handle, metadata)
}

async fn fuse_with(
    strategy: DeduplicationStrategy,
    criteria: GroupingCriteria,
    tree: &DocumentTree<Text>,
    entities: &mut Vec<Entity<Text>>,
) {
    let ctx = LayerContext::new(tree);
    let layer = FuseLayer::new(strategy, criteria);
    let dropped = layer.apply(entities, &ctx).await;
    assert!(dropped.is_empty(), "fuse never drops");
}

#[tokio::test]
async fn confidence_threshold_filters() {
    let tree = tree_from("John......Jane").await;
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
    let ctx = LayerContext::new(&tree).with_correlation_id(Uuid::nil());
    let result = pipeline.run(entities, &ctx).await;
    assert_eq!(result.len(), 1);
    let value = tree.text_at(&result[0].location).await;
    assert_eq!(value.as_deref(), Some("John"));
}

#[tokio::test]
async fn full_pipeline() {
    let tree = tree_from(TEXT).await;
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
    let ctx = LayerContext::new(&tree).with_correlation_id(Uuid::nil());
    let result = pipeline.run(entities, &ctx).await;
    assert_eq!(result.len(), 1);
    assert!((result[0].confidence.get() - 0.85).abs() < f64::EPSILON);
}

#[tokio::test]
async fn strict_grouping_fuses_identical_spans_with_max_confidence() {
    let tree = tree_from(TEXT).await;
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4)
            .with_confidence(conf(0.8))
            .test_build(),
        Entity::test_builder(0, 4).test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::MaxConfidence,
        GroupingCriteria::Strict,
        &tree,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    assert!((entities[0].confidence.get() - 0.9).abs() < f64::EPSILON);
}

#[tokio::test]
async fn narrowing_groups_substring_with_overlap() {
    let tree = tree_from(TEXT).await;
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
        &tree,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    let value = tree.text_at(&entities[0].location).await;
    assert_eq!(value.as_deref(), Some("John Smith"));
}

#[tokio::test]
async fn widening_groups_across_non_overlapping_locations() {
    let text = format!("{:<100}John Smith", TEXT);
    let tree = tree_from(&text).await;
    let mut entities: Vec<_> = vec![
        Entity::test_builder(0, 4).test_build(),
        Entity::test_builder(100, 110)
            .with_trail(vec![ner_step(conf(0.9))])
            .test_build(),
    ];
    fuse_with(
        DeduplicationStrategy::MaxConfidence,
        GroupingCriteria::Widening,
        &tree,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
}

#[tokio::test]
async fn noisy_or_strategy() {
    let tree = tree_from(TEXT).await;
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
        &tree,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    // 1 - (1 - 0.7)(1 - 0.8) = 1 - 0.06 = 0.94
    assert!((entities[0].confidence.get() - 0.94).abs() < 0.001);
}

#[tokio::test]
async fn weighted_average_strategy() {
    let tree = tree_from(TEXT).await;
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
        &tree,
        &mut entities,
    )
    .await;
    assert_eq!(entities.len(), 1);
    // (0.6 * 1.0 + 0.9 * 2.0) / 3.0 = 0.8
    assert!((entities[0].confidence.get() - 0.8).abs() < 0.001);
}

#[tokio::test]
async fn different_detector_tagged_as_ensemble_fusion() {
    let tree = tree_from(TEXT).await;
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
        &tree,
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
