//! [`ContextEnhancer`]: post-recognition keyword-boost pass for
//! any [`Entity<Text>`] regardless of which recognizer produced it.

use derive_builder::{Builder, UninitializedFieldError};

use super::matcher::{KeywordMatcher, SubstringMatcher};
use super::registry::ContextRegistry;
use crate::entity::{Entity, TrailStep};
use crate::modality::Text;
use crate::nlp::NlpArtifacts;
use crate::primitive::Confidence;

/// Post-recognition enhancer that boosts entity confidence when
/// keywords declared by the source recognizer appear near the match.
///
/// Construct via [`builder`]. The two required
/// settings are [`default_window`] (in source-text bytes on each
/// side of the match) and [`default_boost`] (the additive bump
/// applied when a keyword fires). Per-source overrides on
/// [`Context::window`] / [`Context::boost`] take precedence.
///
/// The matcher strategy defaults to [`SubstringMatcher`] when not
/// supplied. Wire [`LemmaMatcher`] instead
/// when the orchestrator produces
/// [`NlpArtifacts`] with lemmas and you
/// want morphological-variant boosting.
///
/// [`builder`]: Self::builder
/// [`Context::window`]: super::Context::window
/// [`Context::boost`]: super::Context::boost
/// [`default_window`]: ContextEnhancerBuilder::with_default_window
/// [`default_boost`]: ContextEnhancerBuilder::with_default_boost
/// [`LemmaMatcher`]: super::LemmaMatcher
/// [`NlpArtifacts`]: crate::nlp::NlpArtifacts
#[derive(Builder)]
#[builder(
    name = "ContextEnhancerBuilder",
    pattern = "owned",
    setter(prefix = "with"),
    build_fn(error = "ContextEnhancerBuilderError")
)]
pub struct ContextEnhancer {
    /// Lookup table built at construction time. The enhancer reads
    /// the source-recognizer / rule name off the entity's first
    /// recognition step and looks it up here to find the declared
    /// [`Context`].
    ///
    /// [`Context`]: super::Context
    #[builder(setter(custom))]
    registry: ContextRegistry,
    /// Keyword-matching strategy (substring, lemma, custom).
    /// Defaults to [`SubstringMatcher`] when omitted.
    #[builder(
        setter(custom),
        default = "Box::new(SubstringMatcher) as Box<dyn KeywordMatcher>"
    )]
    matcher: Box<dyn KeywordMatcher>,
    /// Default window radius (in source-text bytes on each side of
    /// the match). Per-source [`Context::window`] overrides this.
    ///
    /// [`Context::window`]: super::Context::window
    default_window: usize,
    /// Default additive boost applied when a keyword fires.
    /// Per-source [`Context::boost`] overrides this.
    ///
    /// [`Context::boost`]: super::Context::boost
    default_boost: f64,
}

impl ContextEnhancer {
    /// Start building a `ContextEnhancer`. Required:
    /// [`with_registry`],
    /// [`with_default_window`],
    /// [`with_default_boost`].
    ///
    /// [`with_registry`]: ContextEnhancerBuilder::with_registry
    /// [`with_default_window`]: ContextEnhancerBuilder::with_default_window
    /// [`with_default_boost`]: ContextEnhancerBuilder::with_default_boost
    #[must_use]
    pub fn builder() -> ContextEnhancerBuilder {
        ContextEnhancerBuilder::default()
    }

    /// Apply context-keyword boosting to `entities` in place.
    ///
    /// For each entity, looks at its first recognition step's
    /// provenance to identify the source name, looks the name up
    /// in the [`ContextRegistry`], walks the surrounding window
    /// (token-based when `artifacts` is `Some` and the matcher
    /// uses tokens, substring-based otherwise), and bumps the
    /// confidence by the configured boost — capped at `1.0`. A
    /// [`Refinement`]
    /// step is appended to the trail, and the recognition step's
    /// `contextual` flag is set.
    ///
    /// Entities whose source isn't in the registry (or whose
    /// declared context has an empty keyword list) pass through
    /// unchanged.
    ///
    /// [`Refinement`]: crate::entity::TrailStepKind::Refinement
    pub fn enhance(
        &self,
        entities: &mut [Entity<Text>],
        text: &str,
        artifacts: Option<&NlpArtifacts>,
    ) {
        for entity in entities.iter_mut() {
            self.enhance_one(entity, text, artifacts);
        }
    }

    fn enhance_one(&self, entity: &mut Entity<Text>, text: &str, artifacts: Option<&NlpArtifacts>) {
        let Some(name) = entity
            .trail
            .first()
            .and_then(|s| s.provenance.name())
            .map(str::to_owned)
        else {
            return;
        };
        let Some(ctx) = self.registry.get(&name) else {
            return;
        };
        if ctx.keywords.is_empty() {
            return;
        }
        let window = ctx.window.unwrap_or(self.default_window);
        let boost = ctx.boost.unwrap_or(self.default_boost);

        let start = entity.location.start;
        let end = entity.location.end;
        let snippet = window_around(text, start, end, window);
        let tokens_in_window = artifacts.map(|a| {
            // Build a temporary owning `Tokens` from the in-window
            // slice so the matcher's `Option<&Tokens>` signature is
            // honored without allocating a new collection. Use the
            // slice via the `around` helper.
            a.tokens.around(start..end, window)
        });
        // The matcher reads tokens by reference; we hand it the
        // owned-sequence form by wrapping the slice into a temporary
        // `Tokens` only when needed.
        let owned_tokens;
        let tokens_arg = match tokens_in_window {
            Some(slice) if !slice.is_empty() => {
                owned_tokens = crate::nlp::Tokens::new(slice.to_vec());
                Some(&owned_tokens)
            }
            _ => None,
        };
        if !self.matcher.any_match(snippet, tokens_arg, &ctx.keywords) {
            return;
        }

        let original = entity.confidence;
        let adjusted_raw = (original.get() + boost).clamp(0.0, 1.0);
        let Some(adjusted) = Confidence::new(adjusted_raw) else {
            return;
        };
        entity.confidence = adjusted;

        if let Some(step) = entity.trail.first_mut() {
            step.provenance.mark_contextual();
        }

        entity.trail.push(TrailStep::refinement(
            "context-enhancer",
            original,
            adjusted,
            format!("context keyword near `{name}` (+{boost})"),
        ));
    }
}

/// Borrow a `window`-radius slice of `text` centered on the entity
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
    /// Attach the [`ContextRegistry`] the enhancer reads at boost
    /// time. Required.
    #[must_use]
    pub fn with_registry(mut self, registry: ContextRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Override the keyword-matching strategy. Defaults to
    /// [`SubstringMatcher`].
    #[must_use]
    pub fn with_matcher<M: KeywordMatcher + 'static>(mut self, matcher: M) -> Self {
        self.matcher = Some(Box::new(matcher));
        self
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
    use super::*;
    use crate::context::Context;
    use crate::entity::{
        EntityKind, ModelProvenance, PatternProvenance, TrailProvenance, TrailStepKind,
    };
    use crate::modality::Text;

    fn pattern_entity(name: &str, span: std::ops::Range<usize>) -> Entity<Text> {
        let confidence = Confidence::new(0.6).unwrap();
        let provenance = TrailProvenance::Pattern(PatternProvenance::Regex {
            name: name.to_owned(),
            regex: None,
            validator: None,
            contextual: false,
        });
        let step = TrailStep::recognition(
            "pattern",
            confidence,
            provenance,
            format!("pattern `{name}` matched"),
        );
        Entity::builder()
            .with_entity_kind(EntityKind::GovernmentId)
            .with_trail(vec![step])
            .with_confidence(confidence)
            .with_location(Text::new(span.start, span.end))
            .build()
            .expect("entity builds")
    }

    fn model_entity(name: &str, span: std::ops::Range<usize>) -> Entity<Text> {
        let confidence = Confidence::new(0.5).unwrap();
        let provenance = TrailProvenance::Model(ModelProvenance::new(name));
        let step = TrailStep::recognition(
            "ner",
            confidence,
            provenance,
            format!("model `{name}` matched"),
        );
        Entity::builder()
            .with_entity_kind(EntityKind::PersonName)
            .with_trail(vec![step])
            .with_confidence(confidence)
            .with_location(Text::new(span.start, span.end))
            .build()
            .expect("entity builds")
    }

    fn enhancer(registry: ContextRegistry) -> ContextEnhancer {
        ContextEnhancer::builder()
            .with_registry(registry)
            .with_default_window(80)
            .with_default_boost(0.2)
            .build()
            .expect("enhancer builds")
    }

    #[test]
    fn boosts_pattern_entity_when_keyword_near() {
        let registry =
            ContextRegistry::new().with_entry("ssn", Context::new(["ssn", "social security"]));
        let enhancer = enhancer(registry);
        let text = "Your SSN: 123-45-6789";
        let mut entities = vec![pattern_entity("ssn", 10..21)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert!(entities[0].confidence.get() > before);
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
        assert!(contextual);
    }

    #[test]
    fn boosts_model_entity_when_keyword_near() {
        let registry =
            ContextRegistry::new().with_entry("gliner", Context::new(["named", "called", "mr"]));
        let enhancer = enhancer(registry);
        let text = "Mr. Smith is named in the report.";
        let mut entities = vec![model_entity("gliner", 4..9)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert!(entities[0].confidence.get() > before);
        let TrailProvenance::Model(prov) = &entities[0].trail[0].provenance else {
            panic!("expected model provenance");
        };
        assert!(prov.contextual);
    }

    #[test]
    fn skips_entity_with_no_registered_source() {
        let registry = ContextRegistry::new();
        let enhancer = enhancer(registry);
        let text = "Your SSN: 123-45-6789";
        let mut entities = vec![pattern_entity("ssn", 10..21)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert_eq!(entities[0].confidence.get(), before);
    }

    #[test]
    fn per_source_window_overrides_default() {
        let registry =
            ContextRegistry::new().with_entry("far", Context::new(["far_keyword"]).with_window(5));
        let enhancer = enhancer(registry);
        let text = "far_keyword                            XYZ here";
        let mut entities = vec![pattern_entity("far", 39..42)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert_eq!(entities[0].confidence.get(), before);
    }

    #[test]
    fn boost_caps_at_one() {
        let registry =
            ContextRegistry::new().with_entry("high", Context::new(["here"]).with_boost(0.9));
        let enhancer = enhancer(registry);
        let text = "the value is right here in plain sight";
        let mut entity = pattern_entity("high", 16..21);
        // Push base confidence to 0.95
        entity.confidence = Confidence::new(0.95).unwrap();
        let mut entities = vec![entity];
        enhancer.enhance(&mut entities, text, None);
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }
}
