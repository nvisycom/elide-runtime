//! [`EngineBuilder`] — type-state builder for [`Engine`].
//!
//! The builder uses two phantom type parameters to track which
//! required components have been attached. [`build`] is only
//! callable on the fully-configured shape, so missing-required-field
//! errors are compile-time, not runtime.
//!
//! [`build`]: EngineBuilder::build

use std::marker::PhantomData;
use std::sync::Arc;

use super::Engine;
use crate::language::LanguageDetector;
use crate::ner::NerBackend;
use crate::tokenizer::Tokenizer;

/// Phantom marker: NER backend not attached.
pub struct NoNer;
/// Phantom marker: NER backend attached.
pub struct WithNer;
/// Phantom marker: language detector not attached.
pub struct NoLang;
/// Phantom marker: language detector attached.
pub struct WithLang;

/// Type-state builder for [`Engine`].
///
/// `Ner` tracks whether [`with_ner`] has been called; `Lang` tracks
/// [`with_language_detector`]. [`build`] is only implemented when
/// both are `With*` — calling it earlier is a compile error rather
/// than a runtime panic.
///
/// [`with_ner`]: EngineBuilder::with_ner
/// [`with_language_detector`]: EngineBuilder::with_language_detector
/// [`build`]: EngineBuilder::build
pub struct EngineBuilder<Ner = NoNer, Lang = NoLang> {
    ner: Option<Arc<dyn NerBackend>>,
    language: Option<Arc<dyn LanguageDetector>>,
    tokenizer: Option<Arc<dyn Tokenizer>>,
    _marker: PhantomData<(Ner, Lang)>,
}

impl Default for EngineBuilder<NoNer, NoLang> {
    fn default() -> Self {
        Self {
            ner: None,
            language: None,
            tokenizer: None,
            _marker: PhantomData,
        }
    }
}

impl<Ner, Lang> EngineBuilder<Ner, Lang> {
    fn into_state<Ner2, Lang2>(self) -> EngineBuilder<Ner2, Lang2> {
        EngineBuilder {
            ner: self.ner,
            language: self.language,
            tokenizer: self.tokenizer,
            _marker: PhantomData,
        }
    }

    /// Attach a tokenizer. Optional, can be called at any stage.
    pub fn with_tokenizer<T>(mut self, tokenizer: T) -> Self
    where
        T: Tokenizer + 'static,
    {
        self.tokenizer = Some(Arc::new(tokenizer));
        self
    }
}

impl<Lang> EngineBuilder<NoNer, Lang> {
    /// Attach the NER backend. Required; advances the type state.
    pub fn with_ner<B>(mut self, backend: B) -> EngineBuilder<WithNer, Lang>
    where
        B: NerBackend + 'static,
    {
        self.ner = Some(Arc::new(backend));
        self.into_state()
    }
}

impl<Ner> EngineBuilder<Ner, NoLang> {
    /// Attach the language detector. Required; advances the type
    /// state.
    pub fn with_language_detector<D>(mut self, detector: D) -> EngineBuilder<Ner, WithLang>
    where
        D: LanguageDetector + 'static,
    {
        self.language = Some(Arc::new(detector));
        self.into_state()
    }
}

impl EngineBuilder<WithNer, WithLang> {
    /// Build the engine. Only callable once both required
    /// components are attached — earlier calls don't compile.
    pub fn build(self) -> Engine {
        Engine {
            ner: self.ner.expect("type-state guarantees ner is Some"),
            language: self
                .language
                .expect("type-state guarantees language is Some"),
            tokenizer: self.tokenizer,
        }
    }
}
