//! [`PatternRecognizer`]: regex + dictionary detection over
//! [`nvisy_pattern::PatternEngine`].
//!
//! Wraps either the shared `PatternEngine::instance()` singleton
//! (when default-shaped) or an owned engine built from custom
//! config. Forwards [`ScanContext`] from each [`DetectionContext`]
//! so allow/deny lists and context hints work per-call.
//!
//! [`ScanContext`]: nvisy_pattern::ScanContext
//! [`DetectionContext`]: crate::DetectionContext

use std::ops::Deref;
use std::sync::Arc;

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::workflow::PatternDetection;
use nvisy_pattern::PatternEngine;

use crate::error::Result;
use crate::{DetectionContext, Recognizer};

/// Pattern-based recognizer.
///
/// Holds either a reference to the global [`PatternEngine`]
/// singleton or an owned engine compiled from custom config; both
/// expose the same `scan_entities` surface via [`Deref`].
pub struct PatternRecognizer {
    engine: PatternEngineRef,
}

impl PatternRecognizer {
    /// Construct from a [`PatternDetection`] graph config. Uses the
    /// shared singleton when the config is unconstrained; builds a
    /// fresh engine otherwise.
    pub fn from_config(cfg: &PatternDetection) -> Self {
        Self {
            engine: PatternEngineRef::from_config(cfg),
        }
    }

    /// Construct from a shared engine. Use when the caller already
    /// has an `Arc<PatternEngine>` they want to inject.
    pub fn from_engine(engine: Arc<PatternEngine>) -> Self {
        Self {
            engine: PatternEngineRef::Arc(engine),
        }
    }

    /// Construct using the lazily-initialised process-wide default
    /// engine (all built-in patterns).
    pub fn shared() -> Self {
        Self {
            engine: PatternEngineRef::Shared(PatternEngine::instance()),
        }
    }
}

#[async_trait]
impl Recognizer for PatternRecognizer {
    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, ctx: &DetectionContext) -> Result<Entities> {
        let mut entities: Entities = self
            .engine
            .scan_entities(&ctx.text, &ctx.scan_context)
            .into_iter()
            .collect();

        if let Some(ref allowed) = ctx.entities {
            entities.retain(|e| allowed.contains(&e.entity_kind));
        }
        if let Some(threshold) = ctx.score_threshold {
            entities.retain(|e| e.confidence.get() >= threshold);
        }

        Ok(entities)
    }
}

/// Holds either a borrowed reference to the global singleton, an
/// `Arc` to a caller-supplied engine, or an owned engine built
/// from custom config.
enum PatternEngineRef {
    /// The process-wide [`PatternEngine::instance`] singleton.
    Shared(&'static PatternEngine),
    /// A caller-supplied shared engine.
    Arc(Arc<PatternEngine>),
    /// A freshly compiled engine carrying this build's custom
    /// configuration.
    Owned(PatternEngine),
}

impl PatternEngineRef {
    fn from_config(cfg: &PatternDetection) -> Self {
        let needs_custom = !cfg.patterns.is_empty()
            || cfg.confidence_threshold.is_some()
            || cfg.filter.as_ref().is_some_and(|f| !f.is_unconstrained());
        if !needs_custom {
            return Self::Shared(PatternEngine::instance());
        }
        let mut builder = PatternEngine::builder();
        if !cfg.patterns.is_empty() {
            let names: Vec<&str> = cfg.patterns.iter().map(String::as_str).collect();
            builder = builder.with_patterns(&names);
        }
        if let Some(threshold) = cfg.confidence_threshold {
            builder = builder.with_confidence_threshold(threshold);
        }
        if let Some(ref filter) = cfg.filter {
            builder = builder.with_filter(filter.clone());
        }
        Self::Owned(builder.build().expect("pattern engine must compile"))
    }
}

impl Deref for PatternEngineRef {
    type Target = PatternEngine;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Shared(e) => e,
            Self::Arc(e) => e,
            Self::Owned(e) => e,
        }
    }
}
