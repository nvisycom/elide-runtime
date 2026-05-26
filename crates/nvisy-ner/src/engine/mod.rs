//! [`NerEngine`] — composes a [`Backend`] and a [`LanguagePolicy`]
//! into a single entrypoint, plus the [`NerEngineBuilder`] that
//! constructs it.
//!
//! [`Backend`]: crate::core::Backend
//! [`LanguagePolicy`]: crate::language::LanguagePolicy

mod artifacts;

use std::sync::Arc;

use derive_builder::Builder;
use nvisy_core::Result;
use nvisy_ontology::primitive::{LanguageDetection, LanguageProvenance};

pub use self::artifacts::Artifacts;
use crate::core::{Backend, NerContext, NerParams};
use crate::language::{DynLanguagePolicy, LanguagePolicy};

/// Composite NER engine.
///
/// Holds a [`Backend`] and a [`LanguagePolicy`] (both required).
/// The [`analyze`] entrypoint resolves the language (asserted or
/// detected) and then runs NER with the language as a hint.
///
/// Construct via [`builder`]. Per-call options (asserted language,
/// candidate languages, allowed entity kinds, correlation id) ride
/// on [`NerContext`].
///
/// `Clone` is cheap — every field is already an `Arc`, so cloning is
/// two refcount bumps. Pass the engine by value across tasks and
/// keep it in `State` containers freely.
///
/// [`Backend`]: crate::core::Backend
/// [`LanguagePolicy`]: crate::language::LanguagePolicy
/// [`analyze`]: Self::analyze
/// [`builder`]: Self::builder
#[derive(Clone, Builder)]
#[builder(
    name = "NerEngineBuilder",
    pattern = "owned",
    build_fn(error = "NerEngineBuilderError")
)]
pub struct NerEngine {
    #[builder(setter(custom))]
    pub(super) ner: Arc<dyn Backend>,
    #[builder(setter(custom))]
    pub(super) language: Arc<dyn DynLanguagePolicy>,
}

impl NerEngineBuilder {
    /// Attach the NER backend. Required.
    pub fn with_ner_backend<B>(mut self, backend: B) -> Self
    where
        B: Backend + 'static,
    {
        self.ner = Some(Arc::new(backend));
        self
    }

    /// Attach the language-detection policy. Required.
    ///
    /// The engine asks the policy for a fresh detector each
    /// [`analyze`] call, restricted to whatever language scope the
    /// caller supplied via [`NerContext::candidate_languages`].
    ///
    /// [`analyze`]: NerEngine::analyze
    pub fn with_language_policy<P>(mut self, policy: P) -> Self
    where
        P: LanguagePolicy + 'static,
    {
        self.language = Some(Arc::new(policy));
        self
    }
}

/// Error returned by [`NerEngineBuilder::build`] when a required
/// component (NER backend or language policy) is missing.
#[derive(Debug, thiserror::Error)]
#[error("NerEngine build failed: {0}")]
pub struct NerEngineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for NerEngineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        let field = err.field_name();
        let component = match field {
            "language" => "language policy",
            other => other,
        };
        Self(format!("missing required component `{component}`"))
    }
}

impl NerEngine {
    /// Start building an engine.
    pub fn builder() -> NerEngineBuilder {
        NerEngineBuilder::default()
    }

    /// Run all configured components against `text` with `context`.
    ///
    /// Post-filtering by the `entities` allowlist is the caller's
    /// responsibility — `analyze` returns whatever the configured
    /// backend produced. Routed callers (the engine-side
    /// `NerRecognizer`) defer to the central detection-layer filter.
    pub async fn analyze(&self, text: &str, context: &NerContext) -> Result<Artifacts> {
        use tracing::Instrument;

        let span = tracing::debug_span!(
            "nvisy_ner::analyze",
            correlation_id = context.correlation_id.as_ref().map(|id| id.to_string()),
        );

        async move {
            let detections = self.resolve_language(text, context)?;
            let mut params = NerParams::new();
            if let Some(lang) = detections.first().map(|d| &d.language) {
                params = params.with_language(lang);
            }
            if let Some(kinds) = context.entities.as_deref() {
                params = params.with_requested_kinds(kinds);
            }
            if let Some(id) = context.correlation_id {
                params = params.with_correlation_id(id);
            }
            let entities = self.ner.recognize(text, params).await?;

            Ok(Artifacts {
                entities,
                languages: detections,
            })
        }
        .instrument(span)
        .await
    }

    fn resolve_language(&self, text: &str, context: &NerContext) -> Result<Vec<LanguageDetection>> {
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

impl std::fmt::Debug for NerEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NerEngine").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::primitive::LanguageTag;
    use uuid::Uuid;

    use super::*;
    use crate::backend::NoopBackend;
    use crate::language::LinguaLanguagePolicy;

    fn english_engine() -> NerEngine {
        NerEngine::builder()
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
                &NerContext::default(),
            )
            .await
            .unwrap();
        assert!(out.entities.is_empty());
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[tokio::test]
    async fn asserted_language_bypasses_detection() {
        let engine = english_engine();
        let asserted: LanguageTag = "de".parse().unwrap();
        let ctx = NerContext::builder()
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
        let ctx = NerContext::builder()
            .with_correlation_id(id)
            .build()
            .unwrap();
        let out = engine.analyze("The quick brown fox", &ctx).await.unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[test]
    fn engine_builder_errors_when_ner_missing() {
        let err = NerEngine::builder()
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
        let err = NerEngine::builder()
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
        let engine = NerEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap();

        let en: LanguageTag = "en".parse().unwrap();
        let ctx = NerContext::builder()
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
        let engine = NerEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap();

        let de: LanguageTag = "de".parse().unwrap();
        let ctx = NerContext::builder()
            .with_candidate_languages(vec![de])
            .build()
            .unwrap();
        let out = engine
            .analyze("The quick brown fox jumps over the lazy dog.", &ctx)
            .await
            .unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }
}
