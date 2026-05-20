//! [`DetectionEngine`]: orchestrates a list of [`Recognizer`]s
//! against a [`DetectionContext`].

mod context;

use std::sync::Arc;

use derive_builder::Builder;
use nvisy_ontology::entity::Entities;

pub use self::context::{DetectionContext, DetectionContextBuilder, DetectionContextBuilderError};
use crate::Recognizer;
use crate::error::Result;

const TARGET: &str = "nvisy_detection::engine";

/// Composite detection engine.
///
/// Holds an ordered list of [`Recognizer`]s and runs each against
/// a [`DetectionContext`], returning all detected entities
/// combined into a single [`Entities`] collection.
///
/// Recognizers run sequentially. Future work may parallelise them
/// (each holds an `Arc` so contention is on the underlying
/// backends, not the engine), but the order is currently
/// deterministic to make tracing easy to read.
///
/// Dedup, conflict resolution, and threshold filtering are *not*
/// the engine's concern — those live in the downstream pipeline
/// (`nvisy-engine::operation::deduplication`).
///
/// Construct via [`builder`]. At least one recognizer must be
/// attached; calling [`build`] without one returns a
/// `Misconfigured` error.
///
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

    /// Add a recognizer that's already in an `Arc`. Useful when
    /// the same recognizer instance is shared across engines.
    pub fn with_recognizer_arc(mut self, recognizer: Arc<dyn Recognizer>) -> Self {
        self.recognizers
            .get_or_insert_with(Vec::new)
            .push(recognizer);
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

    /// Run every attached recognizer against `ctx` and return the
    /// combined entity set.
    ///
    /// Recognizer offsets are context-local. The caller (typically
    /// `nvisy-engine`'s `Detection` operation) rebases them onto
    /// document coordinates after this returns.
    pub async fn detect(&self, ctx: &DetectionContext<'_>) -> Result<Entities> {
        use tracing::Instrument;

        let span = tracing::debug_span!(
            target: TARGET,
            "detect",
            recognizers = self.recognizers.len(),
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        async move {
            let mut all = Entities::new();
            for recognizer in &self.recognizers {
                let name = recognizer.name();
                let entities = recognizer.recognize(ctx).await?;
                tracing::debug!(
                    target: TARGET,
                    recognizer = name,
                    detected = entities.len(),
                    "recognizer produced entities",
                );
                all.extend(entities);
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

impl std::fmt::Debug for DetectionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectionEngine")
            .field("recognizers", &self.recognizers.len())
            .finish_non_exhaustive()
    }
}
