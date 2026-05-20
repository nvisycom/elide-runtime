//! [`NlpEngine`] — composite that orchestrates a [`NerBackend`], a
//! [`LanguageDetector`], and an optional [`Tokenizer`] on every
//! [`analyze`](NlpEngine::analyze) call.
//!
//! [`NerBackend`]: crate::ner::NerBackend
//! [`LanguageDetector`]: crate::language::LanguageDetector
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

use std::collections::HashSet;
use std::sync::Arc;

use nvisy_ontology::primitive::LanguageTag;

use super::NlpEngineBuilder;
use crate::artifacts::{NlpArtifacts, Token};
use crate::error::NlpError;
use crate::language::{LanguageDetection, LanguageDetector};
use crate::ner::NerBackend;
use crate::tokenizer::Tokenizer;

/// Composite NLP engine.
///
/// Holds a [`NerBackend`] and a [`LanguageDetector`] (both required)
/// plus an optional [`Tokenizer`]. The default
/// [`analyze`](Self::analyze) entrypoint matches Microsoft Presidio's
/// `AnalyzerEngine` ordering: detect language, run NER (with the
/// detected language as a hint), tokenize, derive keywords.
///
/// When the caller already knows the language — e.g. a document
/// uploaded with explicit metadata — use
/// [`analyze_in_language`](Self::analyze_in_language) to bypass
/// detection.
///
/// Construct via [`builder`](Self::builder).
///
/// [`NerBackend`]: crate::ner::NerBackend
/// [`LanguageDetector`]: crate::language::LanguageDetector
/// [`Tokenizer`]: crate::tokenizer::Tokenizer
pub struct NlpEngine {
    pub(super) ner: Arc<dyn NerBackend>,
    pub(super) language: Arc<dyn LanguageDetector>,
    pub(super) tokenizer: Option<Arc<dyn Tokenizer>>,
}

impl NlpEngine {
    /// Start building an engine.
    pub fn builder() -> NlpEngineBuilder {
        NlpEngineBuilder::default()
    }

    /// Run all configured components, detecting the language from
    /// `text` first.
    pub async fn analyze(&self, text: &str) -> Result<NlpArtifacts, NlpError> {
        let detection = self.language.detect(text);
        self.run(text, detection).await
    }

    /// Run all configured components with the caller-asserted
    /// `language`, bypassing detection.
    ///
    /// Use this when the language is known a priori (uploaded with
    /// metadata, set by a UI selector, etc.). The asserted language
    /// is attached to [`NlpArtifacts::language`] with `confidence:
    /// None` to mark its provenance as "asserted, not detected".
    pub async fn analyze_in_language(
        &self,
        text: &str,
        language: LanguageTag,
    ) -> Result<NlpArtifacts, NlpError> {
        let detection = Some(LanguageDetection {
            language,
            confidence: None,
        });
        self.run(text, detection).await
    }

    async fn run(
        &self,
        text: &str,
        detection: Option<LanguageDetection>,
    ) -> Result<NlpArtifacts, NlpError> {
        let language_hint = detection.as_ref().map(|d| &d.language);
        let entities = self.ner.recognize(text, language_hint).await?;
        let tokens = match &self.tokenizer {
            Some(t) => Some(t.tokenize(text)?),
            None => None,
        };
        let keywords = tokens.as_deref().map(derive_keywords);
        let language = detection.map(|d| d.language);

        Ok(NlpArtifacts {
            entities,
            language,
            tokens,
            keywords,
        })
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
