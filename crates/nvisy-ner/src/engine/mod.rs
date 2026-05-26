//! [`Recognizer`] — composes a [`Backend`] and a [`LanguagePolicy`]
//! into a single entrypoint, plus the [`RecognizerBuilder`] that
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
use crate::core::{Backend, Context};
use crate::language::{DynLanguagePolicy, LanguagePolicy};

/// Composite NER recognizer.
///
/// Holds a [`Backend`] and a [`LanguagePolicy`] (both required).
/// The [`recognize`] entrypoint resolves the language (asserted or
/// detected) and then runs the backend with that language as a
/// hint.
///
/// Construct via [`builder`]. Per-call options (asserted language,
/// candidate languages, allowed entity kinds, correlation id) ride
/// on [`Context`].
///
/// `Clone` is cheap — every field is already an `Arc`, so cloning is
/// two refcount bumps. Pass the recognizer by value across tasks
/// and keep it in `State` containers freely.
///
/// [`Backend`]: crate::core::Backend
/// [`LanguagePolicy`]: crate::language::LanguagePolicy
/// [`recognize`]: Self::recognize
/// [`builder`]: Self::builder
#[derive(Clone, Builder)]
#[builder(
    name = "RecognizerBuilder",
    pattern = "owned",
    build_fn(error = "RecognizerBuilderError")
)]
pub struct Recognizer {
    #[builder(setter(custom))]
    pub(super) ner: Arc<dyn Backend>,
    #[builder(setter(custom))]
    pub(super) language: Arc<dyn DynLanguagePolicy>,
}

impl RecognizerBuilder {
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
    /// The recognizer asks the policy for a fresh detector each
    /// [`recognize`] call, restricted to whatever language scope
    /// the caller supplied via [`Context::candidate_languages`].
    ///
    /// [`recognize`]: Recognizer::recognize
    pub fn with_language_policy<P>(mut self, policy: P) -> Self
    where
        P: LanguagePolicy + 'static,
    {
        self.language = Some(Arc::new(policy));
        self
    }
}

/// Error returned by [`RecognizerBuilder::build`] when a required
/// component (NER backend or language policy) is missing.
#[derive(Debug, thiserror::Error)]
#[error("Recognizer build failed: {0}")]
pub struct RecognizerBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for RecognizerBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        let field = err.field_name();
        let component = match field {
            "language" => "language policy",
            other => other,
        };
        Self(format!("missing required component `{component}`"))
    }
}

impl Recognizer {
    /// Start building a recognizer.
    pub fn builder() -> RecognizerBuilder {
        RecognizerBuilder::default()
    }

    /// Run all configured components against `text` with `ctx`.
    ///
    /// Post-filtering by the `entity_kinds` allowlist is the
    /// caller's responsibility — `recognize` returns whatever the
    /// configured backend produced. Routed callers (the
    /// engine-side `NerRecognizer`) defer to the central
    /// detection-layer filter.
    pub async fn recognize(&self, text: &str, ctx: &Context) -> Result<Artifacts> {
        use tracing::Instrument;

        let span = tracing::debug_span!(
            "nvisy_ner::recognize",
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        );

        async move {
            let detections = self.resolve_language(text, ctx)?;
            // Substitute the detected language onto the context
            // before handing it to the backend. Falls back to the
            // caller-asserted value when detection produced nothing.
            let detected_language = detections.first().map(|d| d.language.clone());
            let backend_ctx = Context {
                language: detected_language.or_else(|| ctx.language.clone()),
                candidate_languages: ctx.candidate_languages.clone(),
                entity_kinds: ctx.entity_kinds.clone(),
                correlation_id: ctx.correlation_id,
            };
            let entities = self.ner.recognize(text, &backend_ctx).await?;

            Ok(Artifacts {
                entities,
                languages: detections,
            })
        }
        .instrument(span)
        .await
    }

    fn resolve_language(&self, text: &str, ctx: &Context) -> Result<Vec<LanguageDetection>> {
        if let Some(language) = ctx.language.clone() {
            return Ok(vec![LanguageDetection {
                language,
                confidence: None,
                provenance: LanguageProvenance::Asserted,
                span: None,
            }]);
        }
        let detector = match ctx.candidate_languages.as_deref() {
            Some(candidates) if !candidates.is_empty() => self.language.detector_for(candidates),
            _ => self.language.detector_for_all(),
        };
        detector.detect(text)
    }
}

impl std::fmt::Debug for Recognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recognizer").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::primitive::LanguageTag;
    use uuid::Uuid;

    use super::*;
    use crate::backend::NoopBackend;
    use crate::language::LinguaLanguagePolicy;

    fn english_recognizer() -> Recognizer {
        Recognizer::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn recognize_detects_language() {
        let recognizer = english_recognizer();
        let out = recognizer
            .recognize(
                "The quick brown fox jumps over the lazy dog.",
                &Context::default(),
            )
            .await
            .unwrap();
        assert!(out.entities.is_empty());
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[tokio::test]
    async fn asserted_language_bypasses_detection() {
        let recognizer = english_recognizer();
        let asserted: LanguageTag = "de".parse().unwrap();
        let ctx = Context::new().with_language(asserted);
        let out = recognizer
            .recognize("The quick brown fox", &ctx)
            .await
            .unwrap();
        // Caller-asserted German wins over detected English.
        assert_eq!(out.dominant_language().unwrap().primary_language(), "de");
    }

    #[tokio::test]
    async fn correlation_id_is_accepted() {
        let recognizer = english_recognizer();
        let id = Uuid::new_v4();
        let ctx = Context::new().with_correlation_id(id);
        let out = recognizer
            .recognize("The quick brown fox", &ctx)
            .await
            .unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }

    #[test]
    fn builder_errors_when_ner_missing() {
        let err = Recognizer::builder()
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap_err();
        assert!(
            err.to_string().contains("ner"),
            "error should mention `ner`: {err}",
        );
    }

    #[test]
    fn builder_errors_when_language_policy_missing() {
        let err = Recognizer::builder()
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
        let recognizer = Recognizer::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap();

        let en: LanguageTag = "en".parse().unwrap();
        let ctx = Context::new().with_candidate_languages(vec![en]);
        let out = recognizer
            .recognize("The quick brown fox jumps over the lazy dog.", &ctx)
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
        let recognizer = Recognizer::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
            .build()
            .unwrap();

        let de: LanguageTag = "de".parse().unwrap();
        let ctx = Context::new().with_candidate_languages(vec![de]);
        let out = recognizer
            .recognize("The quick brown fox jumps over the lazy dog.", &ctx)
            .await
            .unwrap();
        assert_eq!(out.dominant_language().unwrap().primary_language(), "en");
    }
}
