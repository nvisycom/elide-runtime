//! [`NlpEngine`] — composes a [`NerBackend`] and a [`LanguageDetector`]
//! (both required) with an optional [`Tokenizer`] into a single
//! entrypoint, plus the [`NlpEngineBuilder`] that constructs it.
//!
//! [`NerBackend`]: crate::ner::NerBackend
//! [`LanguageDetector`]: crate::language::LanguageDetector
//! [`Tokenizer`]: crate::tokenizer::Tokenizer

mod builder;
mod nlp_engine;

pub use self::builder::{NlpEngineBuilder, NoLang, NoNer, WithLang, WithNer};
pub use self::nlp_engine::NlpEngine;

#[cfg(test)]
mod tests {
    use nvisy_ontology::primitive::LanguageTag;

    use super::*;
    use crate::language::LinguaLanguageDetector;
    use crate::ner::NoopNerBackend;
    use crate::tokenizer::UnicodeTokenizer;

    fn english_engine() -> NlpEngine {
        let tags = ["en".parse().unwrap()];
        let det = LinguaLanguageDetector::for_languages(&tags).unwrap();
        NlpEngine::builder()
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
        let engine = NlpEngine::builder()
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
