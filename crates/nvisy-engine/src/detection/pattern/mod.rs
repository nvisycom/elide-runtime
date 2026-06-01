//! [`PatternRecognizer`]: regex + dictionary detection over
//! [`PatternEngine`].
//!
//! Wraps either the shared `PatternEngine::instance()` singleton
//! (when default-shaped) or an owned engine built from custom
//! config. Forwards [`PatternContext`] from each detection call so
//! allow/deny lists and context hints work per-call.
//!
//! The [`PatternDetection`] plan-params schema lives in the
//! [`params`] submodule so the pattern runtime crate stays free of
//! plan types.
//!
//! [`PatternEngine`]: nvisy_pattern::PatternEngine
//! [`PatternContext`]: nvisy_pattern::PatternContext

mod params;

use std::ops::Deref;
use std::sync::Arc;

use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;
use nvisy_pattern::PatternEngine;

pub use self::params::PatternDetection;
use crate::detection::TextDetectionContext;
use crate::detection::recognizer::TextRecognizer;

/// Pattern-based recognizer.
///
/// Holds either a reference to the global [`PatternEngine`]
/// singleton or an owned engine compiled from custom config; both
/// expose the same `scan_entities` surface via [`Deref`].
pub struct PatternRecognizer {
    engine: PatternEngineRef,
}

impl PatternRecognizer {
    /// Construct from a [`PatternDetection`] plan config. Uses the
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

#[async_trait::async_trait]
impl TextRecognizer for PatternRecognizer {
    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn recognize(&self, ctx: &TextDetectionContext) -> Result<Vec<Entity<Text>>> {
        let mut pattern_ctx = ctx.scan_context.clone();
        pattern_ctx.correlation_id = ctx.correlation_id;
        Ok(self.engine.scan_text(&ctx.text, &pattern_ctx))
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
        let needs_custom =
            !cfg.patterns.is_empty() || cfg.filter.as_ref().is_some_and(|f| !f.is_unconstrained());
        if !needs_custom {
            return Self::Shared(PatternEngine::instance());
        }
        let mut builder = PatternEngine::builder();
        if !cfg.patterns.is_empty() {
            let names: Vec<&str> = cfg.patterns.iter().map(String::as_str).collect();
            builder = builder.with_patterns(&names);
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

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind};
    use nvisy_pattern::{MatchSource, PatternContext, RegexPattern, RuntimePattern};

    use super::*;

    #[tokio::test]
    async fn extra_patterns_flow_through_recognizer() {
        let recognizer = PatternRecognizer::shared();
        let extra = RuntimePattern::new(
            "internal-invoice",
            MatchSource::Regex(RegexPattern {
                regex: r"\bINV-\d{4}\b".into(),
                validator: None,
                case_sensitive: true,
                confidence: 0.8,
            }),
        )
        .with_category(EntityCategory::Financial)
        .with_kind(EntityKind::PaymentCard);
        let mut ctx = TextDetectionContext::new("See INV-4242 attached");
        ctx.scan_context = PatternContext {
            extra_patterns: vec![extra],
            ..Default::default()
        };

        let entities = recognizer.recognize(&ctx).await.expect("recognize");
        assert!(
            entities
                .iter()
                .any(|e| matches!(e.entity_kind, EntityKind::PaymentCard)),
            "extra_patterns regex should flow through the recognizer and produce an entity",
        );
    }
}
