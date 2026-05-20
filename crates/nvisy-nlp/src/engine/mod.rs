//! [`Engine`] — composes a [`NerBackend`] and a [`LanguageDetector`]
//! (both required) with an optional [`Tokenizer`] into a single
//! entrypoint, plus the [`EngineBuilder`] that constructs it.
//!
//! [`NerBackend`]: crate::ner::NerBackend
//! [`LanguageDetector`]: crate::language::LanguageDetector
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

mod builder;

pub use self::builder::{EngineBuilder, NoLang, NoNer, WithLang, WithNer};

use std::collections::HashSet;
use std::sync::Arc;

use nvisy_ontology::primitive::LanguageTag;

use crate::artifacts::{Artifacts, Token};
use crate::error::Result;
use crate::language::{LanguageDetection, LanguageDetector, LanguageProvenance};
use crate::ner::NerBackend;
use crate::tokenizer::Tokenizer;

/// Composite NLP engine.
///
/// Holds a [`NerBackend`] and a [`LanguageDetector`] (both required)
/// plus an optional [`Tokenizer`]. The default [`analyze`] entrypoint
/// matches Microsoft Presidio's `AnalyzerEngine` ordering: detect
/// language, run NER (with the detected language as a hint),
/// tokenize, derive keywords.
///
/// When the caller already knows the language — e.g. a document
/// uploaded with explicit metadata — use [`analyze_in_language`] to
/// bypass detection.
///
/// Construct via [`builder`].
///
/// [`NerBackend`]: crate::ner::NerBackend
/// [`LanguageDetector`]: crate::language::LanguageDetector
/// [`Tokenizer`]: crate::tokenizer::Tokenizer
/// [`analyze`]: Self::analyze
/// [`analyze_in_language`]: Self::analyze_in_language
/// [`builder`]: Self::builder
pub struct Engine {
    pub(super) ner: Arc<dyn NerBackend>,
    pub(super) language: Arc<dyn LanguageDetector>,
    pub(super) tokenizer: Option<Arc<dyn Tokenizer>>,
}

impl Engine {
    /// Start building an engine.
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    /// Run all configured components, detecting the language from
    /// `text` first.
    pub async fn analyze(&self, text: &str) -> Result<Artifacts> {
        let detection = self.language.detect(text)?;
        self.run(text, detection).await
    }

    /// Run all configured components with the caller-asserted
    /// `language`, bypassing detection.
    ///
    /// Use this when the language is known a priori (uploaded with
    /// metadata, set by a UI selector, etc.). The asserted language
    /// is attached to [`Artifacts::language`] and carries
    /// [`LanguageProvenance::Asserted`] internally so downstream
    /// code can distinguish it from a detector-produced result.
    pub async fn analyze_in_language(
        &self,
        text: &str,
        language: LanguageTag,
    ) -> Result<Artifacts> {
        let detection = Some(LanguageDetection {
            language,
            confidence: None,
            provenance: LanguageProvenance::Asserted,
        });
        self.run(text, detection).await
    }

    async fn run(&self, text: &str, detection: Option<LanguageDetection>) -> Result<Artifacts> {
        let language_hint = detection.as_ref().map(|d| &d.language);
        let entities = self.ner.recognize(text, language_hint).await?;
        let tokens = match &self.tokenizer {    
            Some(t) => Some(t.tokenize(text)?),
            None => None,
        };
        let keywords = tokens.as_deref().map(derive_keywords);
        let language = detection.map(|d| d.language);

        Ok(Artifacts {
            entities,
            language,
            tokens,
            keywords,
        })
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
    use nvisy_ontology::primitive::LanguageTag;

    use super::*;
    use crate::language::LinguaLanguageDetector;
    use crate::ner::NoopNerBackend;
    use crate::tokenizer::UnicodeTokenizer;

    fn english_engine() -> Engine {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        Engine::builder()
            .with_ner(NoopNerBackend)
            .with_language_detector(det)
            .build()
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
    async fn analyze_in_language_bypasses_detection() {
        let engine = english_engine();
        let asserted: LanguageTag = "de".parse().unwrap();
        let out = engine
            .analyze_in_language("The quick brown fox", asserted)
            .await
            .unwrap();
        // Caller-asserted German wins over detected English.
        assert_eq!(out.language.unwrap().primary_language(), "de");
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
            .build();

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
