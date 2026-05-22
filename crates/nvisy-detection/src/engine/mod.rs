//! [`DetectionEngine`]: orchestrates a list of [`Recognizer`]s
//! against a [`DetectionContext`].
//!
//! Also home to [`Detection`] — the workflow config bundle that
//! auto-assembles a `DetectionEngine` via [`Detection::into_engine`].
//! This keeps "graph-config shape" and "runtime engine" in the same
//! module so the assembly pathway is a one-liner: each per-recognizer
//! params type drives its own constructor.

mod context;

use std::fmt;
use std::sync::Arc;

use derive_builder::Builder;
use nvisy_ontology::entity::Entities;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

pub use self::context::{DetectionContext, DetectionContextBuilder, DetectionContextBuilderError};
use crate::Recognizer;
use crate::error::{Error, Result};
use crate::recognizer::{
    DetectionParams, LlmDetection, LlmRecognizer, NerDetection, NerRecognizer, PatternDetection,
    PatternRecognizer,
};

const TARGET: &str = "nvisy_detection::engine";

/// Composite detection engine.
///
/// Holds an ordered list of [`Recognizer`]s and dispatches them in
/// parallel against a shared [`DetectionContext`], returning every
/// detected entity combined into a single [`Entities`] collection.
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
    recognizers: Vec<Arc<dyn Recognizer>>,
}

impl DetectionEngineBuilder {
    /// Add a recognizer to the engine. May be called multiple
    /// times; recognizers run in the order they were attached.
    pub fn with_recognizer<R>(mut self, recognizer: R) -> Self
    where
        R: Recognizer + 'static,
    {
        self.recognizers
            .get_or_insert_with(Vec::new)
            .push(Arc::new(recognizer));
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
            let mut set: JoinSet<Result<Entities>> = JoinSet::new();
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
                        return Err(Error::Recognizer {
                            name: "detection-engine".into(),
                            cause: format!("recognizer task panicked or was cancelled: {join_err}"),
                        });
                    }
                }
            }
            Ok(all)
        }
        .instrument(span)
        .await
    }

    /// Borrow the attached recognizers, in attach order.
    pub fn recognizers(&self) -> &[Arc<dyn Recognizer>] {
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
}

impl fmt::Debug for DetectionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetectionEngine")
            .field("recognizers", &self.recognizers.len())
            .finish_non_exhaustive()
    }
}

/// Unified workflow detection config.
///
/// Every recognizer-specific field is optional — `Some` opts that
/// recognizer in and supplies its params, `None` opts it out. The
/// shared [`params`] field is honored by every recognizer that
/// runs.
///
/// Calling [`into_engine`] auto-assembles a [`DetectionEngine`]
/// with one recognizer per opted-in slot.
///
/// [`params`]: Self::params
/// [`into_engine`]: Self::into_engine
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Detection {
    /// Cross-recognizer hints (entity kinds, confidence threshold)
    /// applied to every recognizer that runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<DetectionParams>,
    /// Opts the NER recognizer in. `None` skips it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<NerDetection>,
    /// Opts the LLM recognizer in. `None` skips it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<LlmDetection>,
    /// Opts the pattern recognizer in. `None` skips it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternDetection>,
}

impl Detection {
    /// Validate every configured sub-section.
    pub fn validate(&self) -> std::result::Result<(), validator::ValidationErrors> {
        use validator::Validate;
        if let Some(ref params) = self.params {
            params.validate()?;
        }
        if let Some(ref pattern) = self.pattern {
            pattern.validate()?;
        }
        Ok(())
    }

    /// Assemble a [`DetectionEngine`] with one recognizer per
    /// opted-in slot.
    ///
    /// # Errors
    ///
    /// Returns an error if any recognizer fails to construct (NER
    /// engine preset fails to build, LLM agent fails to construct),
    /// or if no recognizers are opted in (an empty engine would
    /// have nothing to dispatch).
    pub fn into_engine(self) -> Result<DetectionEngine> {
        let Detection {
            params: _,
            ner,
            llm,
            pattern,
        } = self;
        let mut builder = DetectionEngine::builder();
        if let Some(ner_cfg) = ner {
            builder = builder.with_recognizer(NerRecognizer::from_config(&ner_cfg)?);
        }
        if let Some(pattern_cfg) = pattern {
            builder = builder.with_recognizer(PatternRecognizer::from_config(&pattern_cfg));
        }
        if let Some(llm_cfg) = llm {
            builder = builder.with_recognizer(LlmRecognizer::new(llm_cfg)?);
        }
        builder
            .build()
            .map_err(|e| Error::Misconfigured(e.to_string()))
    }
}
