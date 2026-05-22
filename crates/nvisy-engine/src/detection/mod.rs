//! Detection: [`Recognizer`] trait, [`DetectionEngine`] orchestrator,
//! built-in recognizer adapters (NER, pattern, LLM).
//!
//! This module hosts the recognizer abstraction together with every
//! adapter that implements it. The trait stays in-crate because
//! every implementation today is an engine-side wrapper around a
//! backend (`nvisy-pattern`, `nvisy-nlp`, `nvisy-agent`); backend
//! crates themselves stay shape-agnostic.

mod context;
mod dyn_recognizer;
mod extension;
mod llm;
mod nlp;
mod pattern;
mod recognizer;
mod recognizers;

use std::fmt;
use std::sync::Arc;

use derive_builder::Builder;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
pub use nvisy_pattern::PatternFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

pub use self::context::{DetectionContext, DetectionContextBuilder, DetectionContextBuilderError};
pub use self::dyn_recognizer::DynRecognizer;
pub use self::extension::RebaseEntities;
pub use self::llm::{LlmContext, LlmDetection, LlmRecognizer};
pub use self::nlp::{NlpContext, NlpDetection, NlpRecognizer};
pub use self::pattern::{PatternContext, PatternDetection, PatternRecognizer};
pub use self::recognizer::{Recognizer, RecognizerKind};
pub use self::recognizers::{DetectionSection, Recognizers};

const TARGET: &str = "nvisy_engine::detection";

/// Composite detection engine.
///
/// Holds an ordered list of recognizers (stored as
/// [`DynRecognizer`] trait objects) and dispatches them in parallel
/// against a shared [`DetectionContext`], returning every detected
/// entity combined into a single [`Entities`] collection.
///
/// Parallelism uses [`tokio::task::JoinSet`]: each recognizer runs
/// on its own task so CPU-bound work (ONNX inference inside the NER
/// backend) and I/O-bound work (LLM HTTP calls inside the LLM
/// backend) overlap. The context is wrapped in an [`Arc`] once and
/// shared by every task — the inner [`TextData`] is itself cheap to
/// clone, so fan-out is an atomic increment, not a copy of the
/// source text.
///
/// Failure is fail-fast: on the first task error every other
/// in-flight task is aborted and the error is returned.
///
/// Dedup, conflict resolution, and threshold filtering are *not*
/// the engine's concern — those live in the downstream pipeline
/// (`nvisy-engine::operation::deduplication`).
///
/// Construct via [`builder`]. At least one recognizer must be
/// attached; calling [`build`] without one returns a
/// `Misconfigured` error.
///
/// [`TextData`]: nvisy_codec::handler::TextData
/// [`builder`]: Self::builder
/// [`build`]: DetectionEngineBuilder::build
#[derive(Builder)]
#[builder(
    name = "DetectionEngineBuilder",
    pattern = "owned",
    build_fn(error = "DetectionEngineBuilderError", validate = "Self::validate")
)]
pub struct DetectionEngine {
    #[builder(setter(custom), default)]
    recognizers: Vec<Arc<dyn DynRecognizer>>,
}

impl DetectionEngineBuilder {
    /// Add a recognizer to the engine. May be called multiple
    /// times; recognizers run in the order they were attached.
    ///
    /// Accepts any [`Recognizer`] whose `Context` is convertible
    /// from `&DetectionContext` — the standard set of built-in
    /// recognizers satisfy this via their `From<&DetectionContext>`
    /// impls in this module.
    pub fn with_recognizer<R>(mut self, recognizer: R) -> Self
    where
        R: Recognizer + 'static,
        R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
    {
        self.recognizers
            .get_or_insert_with(Vec::new)
            .push(Arc::new(recognizer));
        self
    }

    /// Attach a recognizer already wrapped in `Arc`. Used by the
    /// startup-time [`Recognizers`] registry to share each
    /// recognizer across many engines without re-wrapping.
    pub fn with_recognizer_arc<R>(mut self, recognizer: Arc<R>) -> Self
    where
        R: Recognizer + 'static,
        R::Context: for<'a> From<&'a DetectionContext> + Send + Sync,
    {
        let dyn_rec: Arc<dyn DynRecognizer> = recognizer;
        self.recognizers.get_or_insert_with(Vec::new).push(dyn_rec);
        self
    }

    fn validate(&self) -> std::result::Result<(), String> {
        match &self.recognizers {
            Some(rs) if !rs.is_empty() => Ok(()),
            _ => Err("at least one recognizer must be attached".into()),
        }
    }
}

/// Error returned by [`DetectionEngineBuilder::build`] when the
/// engine is misconfigured (currently: no recognizers attached).
#[derive(Debug, thiserror::Error)]
#[error("DetectionEngine build failed: {0}")]
pub struct DetectionEngineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for DetectionEngineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required field `{}`", err.field_name()))
    }
}

impl From<String> for DetectionEngineBuilderError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl DetectionEngine {
    /// Start building an engine.
    pub fn builder() -> DetectionEngineBuilder {
        DetectionEngineBuilder::default()
    }

    /// Run every attached recognizer against `ctx` in parallel and
    /// return the combined entity set.
    ///
    /// Each recognizer runs on its own [`tokio::task::JoinSet`]
    /// task. The first error aborts the remaining in-flight tasks
    /// and is returned to the caller (fail-fast). On success the
    /// outputs are merged in completion order — recognizer
    /// independence means order doesn't affect downstream dedup.
    ///
    /// Recognizer offsets are context-local. The caller (typically
    /// `nvisy-engine`'s `Detection` operation) rebases them onto
    /// document coordinates after this returns.
    pub async fn run(&self, ctx: DetectionContext) -> Result<Entities> {
        use tracing::Instrument;

        let span = tracing::debug_span!(
            target: TARGET,
            "detect",
            recognizers = self.recognizers.len(),
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        let ctx = Arc::new(ctx);
        let recognizers = self.recognizers.clone();

        async move {
            let mut set: JoinSet<nvisy_core::Result<Entities>> = JoinSet::new();
            for recognizer in recognizers {
                let ctx = Arc::clone(&ctx);
                set.spawn(async move { recognizer.run(&ctx).await });
            }

            let mut all = Entities::new();
            while let Some(joined) = set.join_next().await {
                match joined {
                    Ok(Ok(entities)) => {
                        tracing::debug!(
                            target: TARGET,
                            detected = entities.len(),
                            "recognizer produced entities",
                        );
                        all.extend(entities);
                    }
                    Ok(Err(e)) => {
                        set.abort_all();
                        return Err(e);
                    }
                    Err(join_err) => {
                        set.abort_all();
                        return Err(nvisy_core::Error::runtime(
                            format!("recognizer task panicked or was cancelled: {join_err}"),
                            "detection-engine",
                            false,
                        ));
                    }
                }
            }
            Ok(all)
        }
        .instrument(span)
        .await
    }

    /// Borrow the attached recognizers, in attach order.
    pub fn recognizers(&self) -> &[Arc<dyn DynRecognizer>] {
        &self.recognizers
    }

    /// Reset per-document state on every attached recognizer.
    /// Stateless recognizers do nothing; the LLM recognizer
    /// clears coreference state. Call at document boundaries.
    pub async fn reset(&self) {
        for recognizer in &self.recognizers {
            recognizer.reset().await;
        }
    }

    /// Run detection against every text span on the envelope's
    /// document, rebase offsets into document coordinates, and
    /// append to `envelope.audit.entities`. Resets per-document
    /// state at the end of the call so the next document starts
    /// clean.
    ///
    /// `cfg` provides the per-call hints (`entity_kinds`,
    /// `confidence_threshold`) carried into each
    /// [`DetectionContext`].
    pub async fn detect_in(
        &self,
        envelope: &mut crate::operation::DocumentEnvelope,
        cfg: &Detection,
    ) -> Result<()> {
        const TARGET: &str = "nvisy_engine::detection::detect_in";
        let spans = envelope.document.collect_text_spans().await;
        if spans.is_empty() {
            return Ok(());
        }

        let run_id = envelope.shared.run_id;
        let mut all = nvisy_ontology::entity::Entities::new();
        for span in &spans {
            let mut ctx = DetectionContext::new(span.data.clone());
            ctx.correlation_id = Some(run_id);
            if !cfg.entity_kinds.is_empty() {
                ctx.entities = Some(cfg.entity_kinds.clone());
            }
            if let Some(threshold) = cfg.confidence_threshold {
                ctx.score_threshold = Some(threshold);
            }
            let detected = self.run(ctx).await?;
            all.extend(detected.rebase_offsets(span));
        }

        tracing::debug!(
            target: TARGET,
            detected = all.len(),
            spans = spans.len(),
            "appending detected entities",
        );
        envelope.add_entities(all).await;

        self.reset().await;
        Ok(())
    }
}

impl fmt::Debug for DetectionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetectionEngine")
            .field("recognizers", &self.recognizers.len())
            .finish_non_exhaustive()
    }
}

/// Workflow detection node — which recognizers to dispatch and the
/// shared per-call hints.
///
/// Recognizer construction lives in [`Recognizers`], built once at
/// engine startup from `[recognizer.*]` config sections. This node
/// only references already-built recognizers by [`RecognizerKind`].
///
/// [`kinds`] is the enable/disable list — empty means no detection
/// runs for this workflow.
///
/// [`entity_kinds`] and [`confidence_threshold`] are the per-call
/// hints honored by every enabled recognizer. Recognizer-specific
/// build config (provider, model, regex set, etc.) lives in
/// `[recognizer.*]` runtime config, never here.
///
/// [`kinds`]: Self::kinds
/// [`entity_kinds`]: Self::entity_kinds
/// [`confidence_threshold`]: Self::confidence_threshold
#[derive(Debug, Clone, Default, PartialEq, validator::Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Which recognizer kinds to enable for this workflow. Empty
    /// disables detection entirely.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<RecognizerKind>,
    /// Entity-kind allowlist applied to every enabled recognizer.
    /// Empty = all kinds permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_kinds: Vec<nvisy_ontology::entity::EntityKind>,
    /// Minimum confidence threshold honored by every recognizer
    /// (0.0..=1.0). `None` disables confidence filtering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub confidence_threshold: Option<f64>,
}

impl Detection {
    /// Validate the configuration.
    pub fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        use validator::Validate;
        Validate::validate(self)
    }

    /// Assemble a [`DetectionEngine`] by picking each enabled kind
    /// from the pre-built [`Recognizers`] registry.
    ///
    /// # Errors
    ///
    /// Returns a validation error if [`kinds`] names a recognizer
    /// whose `[recognizer.*]` section is not configured, or if
    /// [`kinds`] is empty (no recognizers to dispatch).
    ///
    /// [`kinds`]: Self::kinds
    pub fn into_engine(&self, recognizers: &Recognizers) -> Result<DetectionEngine> {
        let mut builder = DetectionEngine::builder();
        for &kind in &self.kinds {
            recognizers.require(kind)?;
            builder = match kind {
                RecognizerKind::Llm => {
                    builder.with_recognizer_arc(Arc::clone(recognizers.llm.as_ref().unwrap()))
                }
                RecognizerKind::Nlp => {
                    builder.with_recognizer_arc(Arc::clone(recognizers.nlp.as_ref().unwrap()))
                }
                RecognizerKind::Pattern => {
                    builder.with_recognizer_arc(Arc::clone(recognizers.pattern.as_ref().unwrap()))
                }
            };
        }
        builder
            .build()
            .map_err(|e| nvisy_core::Error::validation(e.to_string(), "detection-engine"))
    }
}
