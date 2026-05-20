//! [`Engine`] — composes a [`NerBackend`] and a [`LanguageDetector`]
//! (both required) with an optional [`Tokenizer`] into a single
//! entrypoint, plus the [`EngineBuilder`] that constructs it.
//!
//! [`NerBackend`]: crate::ner::NerBackend
//! [`LanguageDetector`]: crate::language::LanguageDetector
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

mod context;

pub use self::context::{Context, ContextBuilder, ContextBuilderError};

use std::collections::HashSet;
use std::sync::Arc;

use derive_builder::Builder;

use crate::artifacts::{Artifacts, Token};
use crate::error::Result;
use crate::language::{LanguageDetection, LanguageDetector, LanguageProvenance};
use crate::ner::NerBackend;
use crate::tokenizer::Tokenizer;

/// Composite NLP engine.
///
/// Holds a [`NerBackend`] and a [`LanguageDetector`] (both required)
/// plus an optional [`Tokenizer`]. The [`analyze`] entrypoint matches
/// Microsoft Presidio's `AnalyzerEngine` ordering: resolve language
/// (asserted or detected), run NER with the language as a hint,
/// tokenize, derive keywords, then post-filter by entity-kind
/// allowlist and confidence threshold.
///
/// Construct via [`builder`]. Per-call options (asserted language,
/// candidate languages, allowed entity kinds, score threshold,
/// correlation id) ride on [`Context`].
///
/// [`NerBackend`]: crate::ner::NerBackend
/// [`LanguageDetector`]: crate::language::LanguageDetector
/// [`Tokenizer`]: crate::tokenizer::Tokenizer
/// [`analyze`]: Self::analyze
/// [`builder`]: Self::builder
#[derive(Builder)]
#[builder(
    name = "EngineBuilder",
    pattern = "owned",
    build_fn(error = "EngineBuilderError")
)]
pub struct Engine {
    #[builder(setter(custom))]
    pub(super) ner: Arc<dyn NerBackend>,
    #[builder(setter(custom))]
    pub(super) language: Arc<dyn LanguageDetector>,
    #[builder(setter(custom), default)]
    pub(super) tokenizer: Option<Arc<dyn Tokenizer>>,
}

impl EngineBuilder {
    /// Attach the NER backend. Required.
    pub fn with_ner<B>(mut self, backend: B) -> Self
    where
        B: NerBackend + 'static,
    {
        self.ner = Some(Arc::new(backend));
        self
    }

    /// Attach the language detector. Required.
    pub fn with_language_detector<D>(mut self, detector: D) -> Self
    where
        D: LanguageDetector + 'static,
    {
        self.language = Some(Arc::new(detector));
        self
    }

    /// Attach a tokenizer. Optional.
    pub fn with_tokenizer<T>(mut self, tokenizer: T) -> Self
    where
        T: Tokenizer + 'static,
    {
        self.tokenizer = Some(Some(Arc::new(tokenizer)));
        self
    }
}

/// Error returned by [`EngineBuilder::build`] when a required
/// component (NER backend or language detector) is missing.
#[derive(Debug, thiserror::Error)]
#[error("Engine build failed: {0}")]
pub struct EngineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for EngineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required component `{}`", err.field_name()))
    }
}

impl Engine {
    /// Start building an engine.
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    /// Run all configured components against `context`.
    ///
    /// Accepts anything convertible into [`Context`] —
    /// `engine.analyze("text")` works via the blanket `From<&str>`
    /// impl, and `engine.analyze(context)` works when the caller
    /// built a context explicitly.
    pub async fn analyze<'a>(&self, context: impl Into<Context<'a>>) -> Result<Artifacts> {
        let context = context.into();
        let _span = tracing::debug_span!(
            "nvisy_nlp::analyze",
            correlation_id = context.correlation_id.as_ref().map(|id| id.to_string()),
        )
        .entered();

        let detections = self.resolve_language(&context)?;
        let language_hint = detections.first().map(|d| &d.language);
        let mut entities = self.ner.recognize(context.text, language_hint).await?;

        if let Some(allowed) = context.entities.as_deref() {
            entities.retain(|e| allowed.contains(&e.entity_kind));
        }
        if let Some(threshold) = context.score_threshold {
            entities.retain(|e| e.confidence.get() >= threshold);
        }

        let tokens = match &self.tokenizer {
            Some(t) => Some(t.tokenize(context.text)?),
            None => None,
        };
        let keywords = tokens.as_deref().map(derive_keywords);
        let language = detections.into_iter().next().map(|d| d.language);

        Ok(Artifacts {
            entities,
            language,
            tokens,
            keywords,
        })
    }

    fn resolve_language(&self, context: &Context<'_>) -> Result<Vec<LanguageDetection>> {
        if let Some(language) = context.language.clone() {
            return Ok(vec![LanguageDetection {
                language,
                confidence: None,
                provenance: LanguageProvenance::Asserted,
                span: None,
            }]);
        }
        match context.candidate_languages.as_deref() {
            Some(candidates) if !candidates.is_empty() => {
                self.language.detect_in(context.text, candidates)
            }
            _ => self.language.detect(context.text),
        }
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("tokenizer", &self.tokenizer.is_some())
            .finish_non_exhaustive()
    }
}

fn derive_keywords(tokens: &[Token]) -> HashSet<String> {
    tokens
        .iter()
        .filter(|t| !t.is_stop)
        .map(|t| t.text.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use nvisy_ontology::entity::{Entities, EntityKind};
    use nvisy_ontology::primitive::LanguageTag;
    use uuid::Uuid;

    use super::*;
    use crate::language::LinguaLanguageDetector;
    use crate::ner::{NerBackend, NoopNerBackend};
    use crate::tokenizer::UnicodeTokenizer;

    /// NER backend that returns a fixed list of entities ignoring
    /// input. Used to exercise the engine's post-filtering.
    struct CannedNerBackend(Entities);

    #[async_trait]
    impl NerBackend for CannedNerBackend {
        async fn recognize(
            &self,
            _text: &str,
            _language: Option<&LanguageTag>,
        ) -> Result<Entities> {
            Ok(self.0.clone())
        }
    }

    fn canned(kind: EntityKind, confidence: f64) -> nvisy_ontology::entity::Entity {
        nvisy_ontology::entity::Entity::test_builder(0, 4)
            .with_entity_kind(kind)
            .with_confidence(
                nvisy_ontology::primitive::Confidence::new(confidence).expect("in range"),
            )
            .test_build()
    }

    fn english_engine() -> Engine {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        Engine::builder()
            .with_ner(NoopNerBackend)
            .with_language_detector(det)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn analyze_detects_language() {
        let engine = english_engine();
        let out = engine
            .analyze("The quick brown fox jumps over the lazy dog.")
            .await
            .unwrap();
        assert!(out.entities.is_empty());
        assert_eq!(out.language.unwrap().primary_language(), "en");
        assert!(out.tokens.is_none());
        assert!(out.keywords.is_none());
    }

    #[tokio::test]
    async fn asserted_language_bypasses_detection() {
        let engine = english_engine();
        let asserted: LanguageTag = "de".parse().unwrap();
        let req = Context::builder()
            .with_text("The quick brown fox")
            .with_language(asserted)
            .build()
            .unwrap();
        let out = engine.analyze(req).await.unwrap();
        // Caller-asserted German wins over detected English.
        assert_eq!(out.language.unwrap().primary_language(), "de");
    }

    #[tokio::test]
    async fn str_into_context_works() {
        let engine = english_engine();
        let out = engine.analyze("The quick brown fox").await.unwrap();
        assert_eq!(out.language.unwrap().primary_language(), "en");
    }

    #[tokio::test]
    async fn correlation_id_is_accepted() {
        let engine = english_engine();
        let id = Uuid::new_v4();
        let ctx = Context::builder()
            .with_text("The quick brown fox")
            .with_correlation_id(id)
            .build()
            .unwrap();
        let out = engine.analyze(ctx).await.unwrap();
        assert_eq!(out.language.unwrap().primary_language(), "en");
    }

    #[test]
    fn builder_errors_when_text_missing() {
        let err = Context::builder().build().unwrap_err();
        assert!(
            err.to_string().contains("text"),
            "error should mention `text`: {err}",
        );
    }

    #[test]
    fn engine_builder_errors_when_ner_missing() {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        let err = Engine::builder()
            .with_language_detector(det)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("ner"),
            "error should mention `ner`: {err}",
        );
    }

    #[test]
    fn engine_builder_errors_when_language_detector_missing() {
        let err = Engine::builder()
            .with_ner(NoopNerBackend)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("language"),
            "error should mention `language`: {err}",
        );
    }

    #[tokio::test]
    async fn entities_allowlist_drops_other_kinds() {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        let canned = CannedNerBackend(Entities::from(vec![
            canned(EntityKind::PersonName, 0.9),
            canned(EntityKind::EmailAddress, 0.9),
        ]));
        let engine = Engine::builder()
            .with_ner(canned)
            .with_language_detector(det)
            .build()
            .unwrap();

        let req = Context::builder()
            .with_text("anything")
            .with_entities(vec![EntityKind::PersonName])
            .build()
            .unwrap();
        let out = engine.analyze(req).await.unwrap();
        assert_eq!(out.entities.len(), 1);
        assert_eq!(out.entities.0[0].entity_kind, EntityKind::PersonName);
    }

    #[tokio::test]
    async fn score_threshold_drops_low_confidence() {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        let canned = CannedNerBackend(Entities::from(vec![
            canned(EntityKind::PersonName, 0.95),
            canned(EntityKind::PersonName, 0.40),
        ]));
        let engine = Engine::builder()
            .with_ner(canned)
            .with_language_detector(det)
            .build()
            .unwrap();

        let req = Context::builder()
            .with_text("anything")
            .with_score_threshold(0.5)
            .build()
            .unwrap();
        let out = engine.analyze(req).await.unwrap();
        assert_eq!(out.entities.len(), 1);
        assert!(out.entities.0[0].confidence.get() >= 0.5);
    }

    #[tokio::test]
    async fn candidate_languages_outside_configured_set_yield_no_detection() {
        // The detector is configured with English only (workspace
        // builds lingua with the `english` feature). Restricting the
        // call to German leaves an empty intersection — detection
        // should return None instead of falling back to English.
        let configured = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&configured).unwrap();
        let engine = Engine::builder()
            .with_ner(NoopNerBackend)
            .with_language_detector(det)
            .build()
            .unwrap();

        let de: LanguageTag = "de".parse().unwrap();
        let req = Context::builder()
            .with_text("The quick brown fox jumps over the lazy dog.")
            .with_candidate_languages(vec![de])
            .build()
            .unwrap();
        let out = engine.analyze(req).await.unwrap();
        assert!(out.language.is_none());
    }

    #[tokio::test]
    async fn candidate_languages_within_configured_set_detect() {
        // Empty / matching candidate set should be a no-op.
        let configured = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&configured).unwrap();
        let engine = Engine::builder()
            .with_ner(NoopNerBackend)
            .with_language_detector(det)
            .build()
            .unwrap();

        let en: LanguageTag = "en".parse().unwrap();
        let req = Context::builder()
            .with_text("The quick brown fox jumps over the lazy dog.")
            .with_candidate_languages(vec![en])
            .build()
            .unwrap();
        let out = engine.analyze(req).await.unwrap();
        assert_eq!(out.language.unwrap().primary_language(), "en");
    }

    #[tokio::test]
    async fn analyze_with_tokenizer_produces_tokens_and_keywords() {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        let lang: LanguageTag = "en".parse().unwrap();
        let tok = UnicodeTokenizer::with_language(&lang).unwrap();
        let engine = Engine::builder()
            .with_ner(NoopNerBackend)
            .with_language_detector(det)
            .with_tokenizer(tok)
            .build()
            .unwrap();

        let out = engine.analyze("The quick brown fox").await.unwrap();
        let tokens = out.tokens.expect("tokens present");
        assert_eq!(tokens.len(), 4);
        let keywords = out.keywords.expect("keywords present");
        assert!(
            !keywords.contains("the"),
            "stopword 'the' should be filtered from keywords",
        );
        assert!(keywords.contains("quick"));
    }
}
