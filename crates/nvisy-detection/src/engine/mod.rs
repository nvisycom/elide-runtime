//! [`DetectionEngine`]: orchestrates a list of [`Recognizer`]s
//! against a [`DetectionContext`].

mod context;

use std::fmt;
use std::sync::Arc;

use derive_builder::Builder;
use nvisy_ontology::entity::Entities;
use tokio::task::JoinSet;

pub use self::context::{DetectionContext, DetectionContextBuilder, DetectionContextBuilderError};
use crate::Recognizer;
use crate::error::{Error, Result};

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
