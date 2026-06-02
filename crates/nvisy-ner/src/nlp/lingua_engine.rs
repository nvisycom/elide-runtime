//! [`LinguaNlpEngine`]: language-only [`NlpEngine`].
//!
//! Produces [`NlpArtifacts`] populated only on the
//! [`languages`](nvisy_core::nlp::NlpArtifacts::languages) field —
//! no tokens, no NER, no stopwords. Used by pattern-only
//! pipelines that still want a resolved language carried on the
//! artifact (for engine-level routing or for downstream
//! language-aware policies).
//!
//! When `process` receives a `hint`, detection is skipped and the
//! hint becomes a single-entry detection with provenance
//! [`Asserted`](nvisy_ontology::primitive::LanguageProvenance::Asserted).

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::nlp::{NlpArtifacts, NlpCapabilities};
use nvisy_ontology::primitive::{LanguageDetection, LanguageProvenance, LanguageTag};

use super::engine::NlpEngine;
use super::lingua_detector::LinguaDetector;

/// Lingua-backed language-only NLP engine.
///
/// Stateless: every `process` call builds a fresh
/// [`LinguaDetector`] for the requested language scope (the
/// candidate set passed at construction, or "all languages"). The
/// scope is locked in at construction time — there is no per-call
/// scope override; pattern pipelines that need different scopes
/// per call should hold multiple engines.
#[derive(Debug, Clone)]
pub struct LinguaNlpEngine {
    candidates: Vec<LanguageTag>,
}

impl LinguaNlpEngine {
    /// Construct an engine that considers every language compiled
    /// into the `lingua` crate's feature set.
    #[must_use]
    pub fn unrestricted() -> Self {
        Self {
            candidates: Vec::new(),
        }
    }

    /// Construct an engine restricted to `candidates`. Tags lingua
    /// doesn't recognise are silently skipped at detector-build
    /// time. An empty input is equivalent to
    /// [`unrestricted`](Self::unrestricted).
    #[must_use]
    pub fn with_candidates(candidates: impl IntoIterator<Item = LanguageTag>) -> Self {
        Self {
            candidates: candidates.into_iter().collect(),
        }
    }

    fn detector(&self) -> LinguaDetector {
        if self.candidates.is_empty() {
            LinguaDetector::for_all_languages()
        } else {
            LinguaDetector::for_languages(&self.candidates)
                .unwrap_or_else(LinguaDetector::for_all_languages)
        }
    }
}

impl Default for LinguaNlpEngine {
    fn default() -> Self {
        Self::unrestricted()
    }
}

#[async_trait]
impl NlpEngine for LinguaNlpEngine {
    fn supported_languages(&self) -> &[LanguageTag] {
        &self.candidates
    }

    fn capabilities(&self) -> NlpCapabilities {
        NlpCapabilities::language_only()
    }

    async fn process(&self, text: &str, hint: Option<&LanguageTag>) -> Result<NlpArtifacts> {
        if let Some(language) = hint {
            return Ok(NlpArtifacts::language_only(vec![LanguageDetection {
                language: language.clone(),
                confidence: None,
                provenance: LanguageProvenance::Asserted,
                span: None,
            }]));
        }
        let detections = self.detector().detect(text)?;
        Ok(NlpArtifacts::language_only(detections))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn detects_english_without_hint() {
        let engine = LinguaNlpEngine::unrestricted();
        let artifacts = engine
            .process("The quick brown fox jumps over the lazy dog.", None)
            .await
            .unwrap();
        let lang = artifacts.dominant_language().expect("language detected");
        assert_eq!(lang.language.primary_language(), "en");
        assert!(matches!(lang.provenance, LanguageProvenance::Detected));
    }

    #[tokio::test]
    async fn asserted_hint_bypasses_detection() {
        let engine = LinguaNlpEngine::unrestricted();
        let asserted: LanguageTag = "de".parse().unwrap();
        let artifacts = engine
            .process("The quick brown fox", Some(&asserted))
            .await
            .unwrap();
        let lang = artifacts.dominant_language().unwrap();
        assert_eq!(lang.language.primary_language(), "de");
        assert!(matches!(lang.provenance, LanguageProvenance::Asserted));
    }

    #[tokio::test]
    async fn capabilities_are_language_only() {
        let engine = LinguaNlpEngine::unrestricted();
        let caps = engine.capabilities();
        assert!(!caps.produces_tokens);
        assert!(!caps.produces_ner);
    }
}
