//! [`ContextEnhancer`]: post-recognition keyword-boost pass.
//!
//! Built from a [`PatternRegistry`] plus a [`KeywordMatcher`]
//! strategy and default window/boost values. At enhance time, walks
//! each entity's first recognition step, looks the originating
//! pattern/dictionary up in the registry, and applies the boost
//! when any keyword appears in the window around the match.
//!
//! Per-pattern overrides on [`Context.window`](super::super::Context::window)
//! and [`Context.boost`](super::super::Context::boost) take
//! precedence over the enhancer's defaults.

use std::collections::HashMap;

use derive_builder::{Builder, UninitializedFieldError};
use nvisy_ontology::entity::{Entity, TrailProvenance, TrailStep};
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::Confidence;

use super::keyword_matcher::{KeywordMatcher, SubstringMatcher};
use crate::recognition::{Context, PatternRegistry};

/// Post-recognition enhancer that boosts entity confidence when
/// declared keywords appear near the match.
#[derive(Builder)]
#[builder(
    name = "ContextEnhancerBuilder",
    pattern = "owned",
    setter(prefix = "with"),
    build_fn(error = "ContextEnhancerBuilderError", validate = "Self::validate")
)]
pub struct ContextEnhancer {
    /// Lookup table built at construction time from the registry's
    /// patterns and dictionaries: pattern/dictionary name →
    /// `Context`. Both compositions share the same name namespace
    /// — collisions are resolved last-wins (registry order).
    #[builder(setter(custom))]
    lookup: HashMap<String, Context>,
    /// Keyword-matching strategy (substring, word-boundary, custom).
    /// Defaults to [`SubstringMatcher`] when omitted.
    #[builder(
        setter(custom),
        default = "Box::new(SubstringMatcher) as Box<dyn KeywordMatcher>"
    )]
    matcher: Box<dyn KeywordMatcher>,
    /// Default window radius (in bytes on each side of the match).
    /// Per-pattern [`Context::window`] overrides this when set.
    default_window: usize,
    /// Default additive boost applied to matches whose keywords are
    /// found. Per-pattern [`Context::boost`] overrides this when
    /// set.
    default_boost: f64,
}

impl ContextEnhancer {
    /// Start building a `ContextEnhancer`. Required:
    /// [`with_registry`](ContextEnhancerBuilder::with_registry),
    /// [`with_default_window`](ContextEnhancerBuilder::with_default_window),
    /// and
    /// [`with_default_boost`](ContextEnhancerBuilder::with_default_boost).
    #[must_use]
    pub fn builder() -> ContextEnhancerBuilder {
        ContextEnhancerBuilder::default()
    }

    /// Apply context-keyword boosting to `entities` in place.
    ///
    /// For each entity, looks at its first recognition step's
    /// [`TrailProvenance`] to identify the source pattern or
    /// dictionary by name, looks up its [`Context`], and — when at
    /// least one keyword is present in the surrounding window —
    /// bumps the entity's confidence by the configured boost,
    /// capped at `1.0`. A
    /// [`Refinement`](nvisy_ontology::entity::TrailStepKind::Refinement)
    /// step is appended to the entity's trail, and the recognition
    /// step's `contextual` flag is set.
    ///
    /// Entities whose source isn't in the registry (no `Context`,
    /// or `keywords` empty) pass through unchanged.
    pub fn enhance(&self, entities: &mut [Entity<Text>], text: &str) {
        for entity in entities.iter_mut() {
            self.enhance_one(entity, text);
        }
    }

    fn enhance_one(&self, entity: &mut Entity<Text>, text: &str) {
        let Some(name) = recognition_source(entity).map(str::to_owned) else {
            return;
        };
        let Some(ctx) = self.lookup.get(&name) else {
            return;
        };
        if ctx.keywords.is_empty() {
            return;
        }
        let window = ctx.window.unwrap_or(self.default_window);
        let boost = ctx.boost.unwrap_or(self.default_boost);

        let snippet = window_around(text, entity.location.start, entity.location.end, window);
        if !self.matcher.any_match(snippet, &ctx.keywords) {
            return;
        }

        let original = entity.confidence;
        let adjusted_raw = (original.get() + boost).clamp(0.0, 1.0);
        let Some(adjusted) = Confidence::new(adjusted_raw) else {
            return;
        };
        entity.confidence = adjusted;

        if let Some(step) = entity.trail.first_mut()
            && let TrailProvenance::Pattern(prov) = &mut step.provenance
        {
            prov.mark_contextual();
        }

        entity.trail.push(TrailStep::refinement(
            "context-enhancer",
            original,
            adjusted,
            format!("context keyword near `{name}` (+{boost})"),
        ));
    }
}

/// Pull the source pattern/dictionary name from the entity's first
/// recognition step. Returns `None` when the entity has no
/// [`PatternProvenance`] on its base step.
fn recognition_source(entity: &Entity<Text>) -> Option<&str> {
    let step = entity.trail.first()?;
    match &step.provenance {
        TrailProvenance::Pattern(p) => p.name(),
        _ => None,
    }
}

/// Borrow a `(window)`-radius slice of `text` centred on the entity
/// location, clamped to the string bounds and snapped to UTF-8
/// character boundaries.
fn window_around(text: &str, start: usize, end: usize, window: usize) -> &str {
    let lo = floor_char_boundary(text, start.saturating_sub(window));
    let hi = ceil_char_boundary(text, end.saturating_add(window).min(text.len()));
    &text[lo..hi]
}

fn floor_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn ceil_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

impl ContextEnhancerBuilder {
    /// Build the lookup table from `registry`. Patterns and
    /// dictionaries that declare a [`Context`] are indexed by their
    /// name; the rest are skipped (the enhancer will not look up
    /// patterns it cannot boost).
    #[must_use]
    pub fn with_registry(mut self, registry: &PatternRegistry) -> Self {
        let mut lookup = HashMap::new();
        for p in registry.patterns() {
            if !p.context.keywords.is_empty() {
                lookup.insert(p.name.clone(), p.context.clone());
            }
        }
        for d in registry.dictionaries() {
            if !d.context.keywords.is_empty() {
                lookup.insert(d.name.clone(), d.context.clone());
            }
        }
        self.lookup = Some(lookup);
        self
    }

    /// Override the keyword-matching strategy. Defaults to
    /// [`SubstringMatcher`].
    #[must_use]
    pub fn with_matcher<M: KeywordMatcher + 'static>(mut self, matcher: M) -> Self {
        self.matcher = Some(Box::new(matcher));
        self
    }

    fn validate(&self) -> Result<(), ContextEnhancerBuilderError> {
        Ok(())
    }
}

/// Error returned by [`ContextEnhancerBuilder::build`].
#[derive(Debug, thiserror::Error)]
#[error("context enhancer build failed: {0}")]
pub struct ContextEnhancerBuilderError(String);

impl From<UninitializedFieldError> for ContextEnhancerBuilderError {
    fn from(err: UninitializedFieldError) -> Self {
        Self(format!("missing field `{}`", err.field_name()))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::{Context as CoreContext, Recognizer, TextData};
    use nvisy_ontology::entity::{EntityKind, PatternProvenance, TrailProvenance, TrailStepKind};

    use super::*;
    use crate::recognition::{
        Context as PatternContext, PatternRecognizer, PatternRegistry, Regex,
    };

    fn ssn_pattern() -> Regex {
        Regex::builder()
            .with_name("ssn")
            .with_entity_kind(EntityKind::GovernmentId)
            .with_regex(r"\b\d{3}-\d{2}-\d{4}\b")
            .with_score(0.9)
            .with_context(PatternContext::new(["social security", "ssn"]))
            .build()
            .expect("ssn pattern builds")
    }

    async fn recognize(text: &str, registry: PatternRegistry) -> Vec<Entity<Text>> {
        let recognizer = PatternRecognizer::builder()
            .with_registry(registry)
            .build()
            .expect("recognizer builds");
        let ctx = CoreContext::new(TextData::new(text.to_owned()));
        recognizer.recognize(&ctx).await.expect("recognize")
    }

    #[tokio::test]
    async fn boost_fires_when_keyword_present() {
        let registry = PatternRegistry::new().with_pattern(ssn_pattern());
        let text = "Your SSN: 123-45-6789";
        let mut entities = recognize(text, registry.clone()).await;
        assert_eq!(entities.len(), 1);
        let before = entities[0].confidence.get();

        let enhancer = ContextEnhancer::builder()
            .with_registry(&registry)
            .with_default_window(150)
            .with_default_boost(0.1)
            .build()
            .expect("enhancer builds");
        enhancer.enhance(&mut entities, text);

        let after = entities[0].confidence.get();
        assert!(after > before, "expected boost, got {before} → {after}");
        assert!(
            entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Refinement))
        );
        let TrailProvenance::Pattern(PatternProvenance::Regex { contextual, .. }) =
            &entities[0].trail[0].provenance
        else {
            panic!("expected regex provenance");
        };
        assert!(contextual, "contextual flag should be set");
    }

    #[tokio::test]
    async fn no_boost_when_keyword_absent() {
        let registry = PatternRegistry::new().with_pattern(ssn_pattern());
        let text = "Number: 123-45-6789 (no context word)";
        let mut entities = recognize(text, registry.clone()).await;
        let before = entities[0].confidence.get();

        let enhancer = ContextEnhancer::builder()
            .with_registry(&registry)
            .with_default_window(150)
            .with_default_boost(0.1)
            .build()
            .expect("enhancer builds");
        enhancer.enhance(&mut entities, text);

        assert_eq!(entities[0].confidence.get(), before);
        assert!(
            !entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Refinement))
        );
    }

    #[tokio::test]
    async fn pattern_window_override_applies() {
        let pattern = Regex::builder()
            .with_name("custom")
            .with_entity_kind(EntityKind::GovernmentId)
            .with_regex(r"\bXYZ\b")
            .with_score(0.5)
            .with_context(PatternContext::new(["far_keyword"]).with_window(5))
            .build()
            .expect("custom pattern builds");
        let registry = PatternRegistry::new().with_pattern(pattern);
        // The keyword is more than 5 bytes away from XYZ → override
        // window should suppress the boost.
        let text = "far_keyword                            XYZ here";
        let mut entities = recognize(text, registry.clone()).await;
        let before = entities[0].confidence.get();

        let enhancer = ContextEnhancer::builder()
            .with_registry(&registry)
            .with_default_window(150)
            .with_default_boost(0.1)
            .build()
            .expect("enhancer builds");
        enhancer.enhance(&mut entities, text);

        assert_eq!(
            entities[0].confidence.get(),
            before,
            "5-byte override window should not see the far keyword"
        );
    }

    #[tokio::test]
    async fn boost_caps_at_one() {
        let pattern = Regex::builder()
            .with_name("ssn-high")
            .with_entity_kind(EntityKind::GovernmentId)
            .with_regex(r"\b\d{3}-\d{2}-\d{4}\b")
            .with_score(0.95)
            .with_context(PatternContext::new(["ssn"]).with_boost(0.5))
            .build()
            .expect("pattern builds");
        let registry = PatternRegistry::new().with_pattern(pattern);
        let text = "Your SSN is 123-45-6789";
        let mut entities = recognize(text, registry.clone()).await;
        let enhancer = ContextEnhancer::builder()
            .with_registry(&registry)
            .with_default_window(150)
            .with_default_boost(0.1)
            .build()
            .expect("enhancer builds");
        enhancer.enhance(&mut entities, text);
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }
}
