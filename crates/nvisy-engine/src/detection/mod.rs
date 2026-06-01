//! Detection phase: composite engine + per-modality dispatch.
//!
//! Public surface is [`DetectionPhase`] plus the engine + plan
//! re-exported from this module. The engine itself, its builder, and
//! the per-modality detection bodies live in the `engine` submodule;
//! this file carries only the plan config struct ([`Detection`]) that
//! the orchestrator hands the phase, plus the phase struct.

mod config;
mod context;
mod engine;
mod lift;
mod llm;
mod ner;
mod plan;
mod recognizer;
mod vlm;

pub use nvisy_agent::agent::LlmNerContext;
use nvisy_core::Result;
pub use nvisy_ner::Context as NerContext;
use tracing::Instrument;

pub use self::config::DetectionConfig;
pub use self::context::{DetectionContext, ImageDetectionContext, TextDetectionContext};
pub use self::engine::DetectionEngine;
pub use self::lift::{LiftFromBlock, ProjectIntoBlock};
pub use self::llm::{
    DetectParams, LlmDetection, LlmRecognizer, VerifyParams,
    build_recognizer as build_llm_recognizer,
};
pub use self::ner::{NerDetection, NerRecognizer};
pub use self::plan::Detection;
pub use self::recognizer::{ImageRecognizer, TextRecognizer, names};
pub use self::vlm::{
    VlmDetectParams, VlmDetection, VlmRecognizer, VlmVerifyParams,
    build_recognizer as build_vlm_recognizer,
};
use crate::core::{DocumentTree, RunContext};
use crate::pipeline::EngineInput;

const TARGET: &str = "nvisy_engine::detection";

/// Detection phase: runs configured recognizers over each node's
/// blocks and writes [`EntityRecord`]s to `doc.audit`.
///
/// Holds a [`DetectionEngine`] by value — the engine's recognizer
/// lists keep the underlying recognizers shared via `Arc` inside,
/// without an outer wrap.
///
/// [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
pub struct DetectionPhase {
    engine: DetectionEngine,
}

impl DetectionPhase {
    /// Build the phase from the shared detection engine. Called once
    /// per pipeline by [`DocumentPipeline::from_context`].
    ///
    /// [`DocumentPipeline::from_context`]: crate::pipeline::DocumentPipeline::from_context
    pub fn new(engine: DetectionEngine) -> Self {
        Self { engine }
    }

    /// Walk the tree and dispatch the right recognizers per modality
    /// node. Visits the root first, then iterates nested embedded
    /// documents; each per-node body borrows the engine, handle, and
    /// detection plan directly from this scope.
    pub(crate) async fn apply(
        &self,
        ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "detection");
        let run_id = ctx.shared().run_id;
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            self.engine
                .dispatch(tree.root_mut(), &handle, &input.plan.detection, run_id)
                .await?;
            for node in tree.embeds_mut() {
                self.engine
                    .dispatch(node, &handle, &input.plan.detection, run_id)
                    .await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }
}
