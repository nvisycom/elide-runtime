//! [`NlpEngineBuilder`] — fluent builder for [`NlpEngine`].

use std::sync::Arc;

use super::NlpEngine;
use crate::language::LanguageDetector;
use crate::ner::NerBackend;
use crate::tokenizer::Tokenizer;

/// Builder for [`NlpEngine`].
///
/// Both the NER backend and the language detector are mandatory.
/// [`build`](Self::build) panics if either is missing.
#[derive(Default)]
pub struct NlpEngineBuilder {
    ner: Option<Arc<dyn NerBackend>>,
    language: Option<Arc<dyn LanguageDetector>>,
    tokenizer: Option<Arc<dyn Tokenizer>>,
}

impl NlpEngineBuilder {
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
        self.tokenizer = Some(Arc::new(tokenizer));
        self
    }

    /// Build the engine.
    ///
    /// # Panics
    ///
    /// Panics if either the NER backend or the language detector was
    /// not attached.
    pub fn build(self) -> NlpEngine {
        NlpEngine {
            ner: self.ner.expect("NlpEngine requires a NerBackend"),
            language: self.language.expect("NlpEngine requires a LanguageDetector"),
            tokenizer: self.tokenizer,
        }
    }
}
