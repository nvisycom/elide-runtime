//! [`NlpEngine`] — composes a [`NerBackend`] and a [`LanguagePolicy`]
//! (both required) with an optional [`Tokenizer`] into a single
//! entrypoint, plus the [`NlpEngineBuilder`] that constructs it.
//!
//! [`NerBackend`]: crate::ner::NerBackend
//! [`LanguagePolicy`]: crate::language::LanguagePolicy
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

mod artifacts;
mod context;

use std::collections::HashSet;
use std::sync::Arc;

use derive_builder::Builder;

pub use self::artifacts::Artifacts;
pub use self::context::{NlpContext, NlpContextBuilder, NlpContextBuilderError};
use crate::error::Result;
use crate::language::{DynLanguagePolicy, LanguageDetection, LanguagePolicy, LanguageProvenance};
use crate::ner::NerBackend;
use crate::tokenizer::{Token, Tokenizer};

/// Composite NLP engine.
///
/// Holds a [`NerBackend`] and a [`LanguagePolicy`] (both required)
/// plus an optional [`Tokenizer`]. The [`analyze`] entrypoint matches
/// Microsoft Presidio's `AnalyzerEngine` ordering: resolve language
/// (asserted or detected), run NER with the language as a hint,
/// tokenize, derive keywords, then post-filter by entity-kind
/// allowlist and confidence threshold.
///
/// Construct via [`builder`]. Per-call options (asserted language,
/// candidate languages, allowed entity kinds, score threshold,
/// correlation id) ride on [`NlpContext`].
///
/// `Clone` is cheap — every field is already an `Arc`, so cloning is
/// three refcount bumps. Pass the engine by value across tasks and
/// keep it in `State` containers freely.
///
/// [`NerBackend`]: crate::ner::NerBackend
/// [`LanguagePolicy`]: crate::language::LanguagePolicy
/// [`Tokenizer`]: crate::tokenizer::Tokenizer
/// [`analyze`]: Self::analyze
/// [`builder`]: Self::builder
#[derive(Clone, Builder)]
#[builder(
    name = "NlpEngineBuilder",
    pattern = "owned",
    build_fn(error = "NlpEngineBuilderError")
)]
pub struct NlpEngine {
    #[builder(setter(custom))]
    pub(super) ner: Arc<dyn NerBackend>,
    #[builder(setter(custom))]
    pub(super) language: Arc<dyn DynLanguagePolicy>,
    #[builder(setter(custom), default)]
    pub(super) tokenizer: Option<Arc<dyn Tokenizer>>,
}

impl NlpEngineBuilder {
    /// Attach the NER backend. Required.
    pub fn with_ner_backend<B>(mut self, backend: B) -> Self
    where
        B: NerBackend + 'static,
    {
        self.ner = Some(Arc::new(backend));
        self
    }

    /// Attach the language-detection policy. Required.
    ///
    /// The engine asks the policy for a fresh detector each
    /// [`analyze`] call, restricted to whatever language scope the
    /// caller supplied via [`NlpContext::candidate_languages`].
    ///
    /// [`analyze`]: NlpEngine::analyze
    pub fn with_language_policy<P>(mut self, policy: P) -> Self
    where
        P: LanguagePolicy + 'static,
    {
        self.language = Some(Arc::new(policy));
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

/// Error returned by [`NlpEngineBuilder::build`] when a required
/// component (NER backend or language policy) is missing.
#[derive(Debug, thiserror::Error)]
#[error("NlpEngine build failed: {0}")]
pub struct NlpEngineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for NlpEngineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        let field = err.field_name();
        // Rename the storage field `language` back to what the
        // public setter is called.
        let component = match field {
            "language" => "language policy",
            other => other,
        };
        Self(format!("missing required component `{component}`"))
    }
}

impl NlpEngine {
    /// Start building an engine.
    pub fn builder() -> NlpEngineBuilder {
        NlpEngineBuilder::default()
    }

    /// Run all configured components against `text` with `context`.
    ///
    /// Post-filtering by `entities` allowlist or `score_threshold`
    /// is the caller's responsibility — `analyze` returns whatever
    /// the configured backend produced. Routed callers (the engine's
    /// `NlpRecognizer`) defer to the central detection-layer filter.
    pub async fn analyze(&self, text: &str, context: &NlpContext) -> Result<Artifacts> {
        use tracing::Instrument;

        let span = tracing::debug_span!(
            "nvisy_nlp::analyze",
            correlation_id = context.correlation_id.as_ref().map(|id| id.to_string()),
        );

        async move {
            let detections = self.resolve_language(text, context)?;
            let language_hint = detections.first().map(|d| &d.language);
            let entities = self
                .ner
                .recognize(text, language_hint, context.entities.as_deref())
                .await?;

            let tokens = match &self.tokenizer {
                Some(t) => Some(t.tokenize(text)?),
                None => None,
            };
            let keywords = tokens.as_deref().map(derive_keywords);

            Ok(Artifacts {
                entities,
                languages: detections,
                tokens,
                keywords,
            })
        }
        .instrument(span)
        .await
    }

    fn resolve_language(&self, text: &str, context: &NlpContext) -> Result<Vec<LanguageDetection>> {
        if let Some(language) = context.language.clone() {
            return Ok(vec![LanguageDetection {
                language,
                confidence: None,
                provenance: LanguageProvenance::Asserted,
                span: None,
            }]);
        }
        let detector = match context.candidate_languages.as_deref() {
            Some(candidates) if !candidates.is_empty() => self.language.detector_for(candidates),
            _ => self.language.detector_for_all(),
        };
        detector.detect(text)
    }
}

impl std::fmt::Debug for NlpEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NlpEngine")
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
    use nvisy_ontology::primitive::LanguageTag;
    use uuid::Uuid;

    use super::*;
    use crate::language::LinguaLanguagePolicy;
    use crate::ner::NoopBackend;
    use crate::tokenizer::UnicodeTokenizer;

    fn english_engine() -> NlpEngine {
        NlpEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn analyze_detects_language() {
        let engine = english_engine();
        let out = engine
            .analyze(
                "The quick brown fox jumps over the lazy dog.",
                &NlpContext::default(),
            )
            .await
            .unwrap();
        assert!(out.entities.is_empty());
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
        assert!(out.tokens.is_none());
        assert!(out.keywords.is_none());
    }

    #[tokio::test]
    async fn asserted_language_bypasses_detection() {
        let engine = english_engine();
        let asserted: LanguageTag = "de".parse().unwrap();
        let ctx = NlpContext::builder()
            .with_language(asserted)
            .build()
            .unwrap();
        let out = engine.analyze("The quick brown fox", &ctx).await.unwrap();
        // Caller-asserted German wins over detected English.
        assert_eq!(out.dominant_language().unwrap().primary_language(), "de");
    }

    #[tokio::test]
    async fn correlation_id_is_accepted() {
        let engine = english_engine();
        let id = Uuid::new_v4();
        let ctx = NlpContext::builder()
            .with_correlation_id(id)
            .build()
            .unwrap();
        let out = engine.analyze("The quick brown fox", &ctx).await.unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[test]
    fn engine_builder_errors_when_ner_missing() {
        let err = NlpEngine::builder()
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("ner"),
            "error should mention `ner`: {err}",
        );
    }

    #[test]
    fn engine_builder_errors_when_language_policy_missing() {
        let err = NlpEngine::builder()
            .with_ner_backend(NoopBackend)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("language policy"),
            "error should mention `language policy`: {err}",
        );
    }

    #[tokio::test]
    async fn candidate_languages_supported_by_policy_detect() {
        // LinguaLanguagePolicy honours candidate_languages by
        // building a detector restricted to them. English is the
        // only language enabled via the workspace's `english`
        // lingua feature, so an English candidate set on English
        // text resolves to English.
        let engine = NlpEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap();

        let en: LanguageTag = "en".parse().unwrap();
        let ctx = NlpContext::builder()
            .with_candidate_languages(vec![en])
            .build()
            .unwrap();
        let out = engine
            .analyze("The quick brown fox jumps over the lazy dog.", &ctx)
            .await
            .unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[tokio::test]
    async fn candidate_languages_unsupported_by_policy_fall_back() {
        // LinguaLanguagePolicy silently skips tags lingua doesn't
        // recognise. With the workspace's English-only feature set
        // and a German-only candidate list, the policy falls back
        // to detector_for_all (which still considers only English),
        // so English text still detects as English.
        let engine = NlpEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap();

        let de: LanguageTag = "de".parse().unwrap();
        let ctx = NlpContext::builder()
            .with_candidate_languages(vec![de])
            .build()
            .unwrap();
        let out = engine
            .analyze("The quick brown fox jumps over the lazy dog.", &ctx)
            .await
            .unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[tokio::test]
    async fn analyze_with_tokenizer_produces_tokens_and_keywords() {
        let lang: LanguageTag = "en".parse().unwrap();
        let tok = UnicodeTokenizer::with_language(&lang).unwrap();
        let engine = NlpEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .with_tokenizer(tok)
            .build()
            .unwrap();

        let out = engine
            .analyze("The quick brown fox", &NlpContext::default())
            .await
            .unwrap();
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
