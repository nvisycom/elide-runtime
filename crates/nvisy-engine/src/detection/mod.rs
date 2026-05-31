//! Detection phase: composite engine + per-modality dispatch.
//!
//! Public surface is [`DetectionPhase`] plus the engine + plan
//! re-exported from this module. The engine itself, its builder, and
//! all per-modality [`DetectDispatch`] impls live in the `engine`
//! submodule; this file carries only the plan config struct
//! ([`Detection`]) that the orchestrator hands the phase, plus the
//! stateless phase wrapper.

mod config;
mod context;
mod dispatch;
mod dyn_recognizer;
mod engine;
mod lift;
mod llm;
mod ner;
mod pattern;
mod recognizer;
mod recognizers;
mod vlm;

use std::marker::PhantomData;
use std::sync::Arc;

pub use nvisy_agent::agent::LlmNerContext;
use nvisy_core::Result;
pub use nvisy_ner::Context as NerContext;
use nvisy_ontology::modality::Modality;
pub use nvisy_pattern::{PatternContext, PatternFilter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

pub use self::config::DetectionConfig;
pub use self::context::{
    DetectionContext, DetectionContextBuilder, DetectionContextBuilderError, VlmDetectionContext,
};
pub use self::dispatch::DetectDispatch;
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
use crate::pipeline::{ModalityKind, Phase, PhaseContext, PhaseInfo, PhaseTarget};

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

/// Detection phase: runs configured recognizers over the target's
/// blocks and writes [`EntityRecord`]s to `target.doc.audit`.
///
/// Stateless beyond the modality marker — the shared
/// [`DetectionEngine`] is read from `ctx.run` each call, and the
/// per-call plan slice comes from `ctx.plan.detection`. Only
/// constructed by the orchestrator when the run has a configured
/// engine; the phase body unwraps that guarantee.
///
/// [`EntityRecord`]: nvisy_ontology::provenance::EntityRecord
pub struct DetectionPhase<M: Modality> {
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> DetectionPhase<M> {
    /// Build the phase. Stateless beyond the modality marker.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<M: Modality> Default for DetectionPhase<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<M> Phase<M> for DetectionPhase<M>
where
    M: Modality,
    DetectionEngine: DetectDispatch<M>,
{
    fn inspect(&self) -> PhaseInfo {
        PhaseInfo {
            name: "detection",
            modality: ModalityKind::of::<M>(),
            mutating: true,
        }
    }

    async fn run(&self, ctx: &PhaseContext<'_, M>, target: &mut PhaseTarget<'_, M>) -> Result<()> {
        DetectDispatch::<M>::detect(
            ctx.run.detection_engine.as_ref(),
            target,
            &ctx.plan.detection,
        )
        .await
    }
}
