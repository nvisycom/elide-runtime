//! Detection phase: composite engine + per-modality dispatch.
//!
//! Public surface is [`DetectionPhase`] plus the engine + plan
//! re-exported from this module. The engine itself, its builder, and
//! the per-modality detection bodies live in the `engine` submodule;
//! this file carries only the plan config struct ([`Detection`]) that
//! the orchestrator hands the phase, plus the phase struct.

mod config;
mod context;
mod dyn_recognizer;
mod engine;
mod lift;
mod llm;
mod ner;
mod pattern;
mod recognizer;
mod recognizers;
mod vlm;

use std::sync::Arc;

pub use nvisy_agent::agent::LlmNerContext;
use nvisy_core::Result;
pub use nvisy_ner::Context as NerContext;
use nvisy_ontology::modality::{Image, Modality, Text};
pub use nvisy_pattern::{PatternContext, PatternFilter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::Instrument;
use validator::Validate;

pub use self::config::DetectionConfig;
pub use self::context::{
    DetectionContext, DetectionContextBuilder, DetectionContextBuilderError, VlmDetectionContext,
};
pub use self::dyn_recognizer::{DynImageRecognizer, DynTextRecognizer};
pub use self::engine::{DetectionEngine, DetectionEngineBuilder, DetectionEngineBuilderError};
pub use self::lift::{LiftFromBlock, ProjectIntoBlock};
pub use self::llm::{
    DetectParams, LlmDetection, VerifyParams, build_pipeline as build_llm_pipeline,
};
pub use self::ner::{NerDetection, NerRecognizer};
pub use self::pattern::{PatternDetection, PatternRecognizer};
pub use self::recognizer::{Recognizer, RecognizerKind};
pub use self::recognizers::{ImageRecognizers, Recognizers, TextRecognizers};
pub use self::vlm::{
    VlmDetectParams, VlmDetection, VlmVerifyParams, build_pipeline as build_vlm_pipeline,
};
use crate::core::{DocumentTree, NodeMut, RunContext, SharedHandle};
use crate::pipeline::EngineInput;

const TARGET: &str = "nvisy_engine::detection";

/// Plan detection node — which recognizers to dispatch and the
/// shared per-call hints.
///
/// Recognizer construction lives in [`Recognizers`], built once at
/// engine startup from `[detection.*]` config sections. This node
/// only references already-built recognizers by [`RecognizerKind`].
///
/// [`kinds`] is the enable/disable list — empty means no detection
/// runs for this plan.
///
/// [`entity_kinds`] is the per-call hint honored by every enabled
/// recognizer. Recognizer-specific build config (provider, model,
/// regex set, including the recognizer's own confidence threshold)
/// lives in `[detection.*]` runtime config, never here.
///
/// Confidence-based filtering is centralised in the deduplication
/// phase, applied once after per-recognizer calibration. There is no
/// per-plan confidence threshold — operators tune trust via the
/// dedup calibration map plus the single dedup threshold.
///
/// [`kinds`]: Self::kinds
/// [`entity_kinds`]: Self::entity_kinds
#[derive(Debug, Clone, Default, PartialEq, validator::Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Which recognizer kinds to enable for this plan. Empty
    /// disables detection entirely.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<RecognizerKind>,
    /// Entity-kind allowlist applied to every enabled recognizer.
    /// Empty = all kinds permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_kinds: Vec<nvisy_ontology::entity::EntityKind>,
}

impl Detection {
    /// Validate the configuration.
    pub fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        Validate::validate(self)
    }

    /// Assemble a [`DetectionEngine`] by picking each enabled kind
    /// from the pre-built [`Recognizers`] registry.
    ///
    /// # Errors
    ///
    /// Returns a validation error if [`kinds`] names a recognizer
    /// whose `[detection.*]` section is not configured, or if
    /// [`kinds`] is empty (no recognizers to dispatch).
    ///
    /// [`kinds`]: Self::kinds
    pub fn into_engine(&self, recognizers: &Recognizers) -> Result<DetectionEngine> {
        let mut builder = DetectionEngine::builder();
        for &kind in &self.kinds {
            recognizers.require(kind)?;
            // Each unwrap is guaranteed by the preceding `require()`
            // call: it returns Err if the slot is None, so reaching
            // these arms means the matching slot is Some.
            builder = match kind {
                RecognizerKind::Llm => builder.with_text_recognizer_arc(Arc::clone(
                    recognizers
                        .text
                        .llm
                        .as_ref()
                        .expect("require(Llm) guarantees text.llm is Some"),
                )),
                RecognizerKind::Ner => builder.with_text_recognizer_arc(Arc::clone(
                    recognizers
                        .text
                        .ner
                        .as_ref()
                        .expect("require(Ner) guarantees text.ner is Some"),
                )),
                RecognizerKind::Pattern => builder.with_text_recognizer_arc(Arc::clone(
                    recognizers
                        .text
                        .pattern
                        .as_ref()
                        .expect("require(Pattern) guarantees text.pattern is Some"),
                )),
                RecognizerKind::Vlm => builder.with_image_recognizer_arc(Arc::clone(
                    recognizers
                        .image
                        .vlm
                        .as_ref()
                        .expect("require(Vlm) guarantees image.vlm is Some"),
                )),
            };
        }
        builder
            .build()
            .map_err(|e| nvisy_core::Error::validation(e.to_string(), "detection-engine"))
    }
}

/// Detection phase: runs configured recognizers over each node's
/// blocks and writes [`EntityRecord`]s to `doc.audit`.
///
/// Holds an `Arc<DetectionEngine>` shared across every run.
///
/// [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
pub struct DetectionPhase {
    engine: Arc<DetectionEngine>,
}

impl DetectionPhase {
    /// Build the phase from the shared detection engine. Called once
    /// per pipeline by [`DocumentPipeline::from_context`].
    ///
    /// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
    pub fn new(engine: Arc<DetectionEngine>) -> Self {
        Self { engine }
    }

    /// Walk the tree and dispatch the right recognizers per modality
    /// node.
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "detection");
        let handle = tree.handle.clone();
        let engine = Arc::clone(&self.engine);
        let cfg = Arc::new(input.plan.detection.clone());
        let run_id = ctx.shared().run_id;
        async move {
            tree.walk_mut(move |node| {
                let engine = Arc::clone(&engine);
                let handle = handle.clone();
                let cfg = Arc::clone(&cfg);
                Box::pin(async move { dispatch(&engine, node, &handle, &cfg, run_id).await })
            })
            .await
        }
        .instrument(span)
        .await
    }
}

async fn dispatch(
    engine: &DetectionEngine,
    node: NodeMut<'_>,
    handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => detect_text_only::<Text>(engine, doc, cfg, run_id).await,
        NodeMut::Tabular(doc) => {
            detect_text_only::<nvisy_ontology::modality::Tabular>(engine, doc, cfg, run_id).await
        }
        NodeMut::Audio(doc) => {
            detect_text_only::<nvisy_ontology::modality::Audio>(engine, doc, cfg, run_id).await
        }
        NodeMut::Image(doc) => detect_image(engine, doc, handle, cfg, run_id).await,
    }
}

async fn detect_text_only<M>(
    engine: &DetectionEngine,
    doc: &mut nvisy_ontology::document::Document<M>,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()>
where
    M: Modality + LiftFromBlock + ProjectIntoBlock + nvisy_ontology::modality::Overlap,
{
    engine.detect_text_only(doc, cfg, run_id).await
}

#[cfg(feature = "image")]
async fn detect_image(
    engine: &DetectionEngine,
    doc: &mut nvisy_ontology::document::Document<Image>,
    handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    // Text recognizers run on every OCR'd block ("runs alongside"
    // image-side recognizers), then image recognizers run once per
    // image location with raw bytes — they emit absolute
    // image-coord Entity<Image> directly, no per-block lifting.
    self::engine::detect_text_blocks(engine, doc, cfg, run_id).await?;
    engine
        .detect_image_locations(doc, handle, cfg, run_id)
        .await?;
    engine.reset().await;
    Ok(())
}

#[cfg(not(feature = "image"))]
async fn detect_image(
    engine: &DetectionEngine,
    doc: &mut nvisy_ontology::document::Document<Image>,
    _handle: &SharedHandle,
    cfg: &Detection,
    run_id: uuid::Uuid,
) -> Result<()> {
    engine.detect_text_only(doc, cfg, run_id).await
}
