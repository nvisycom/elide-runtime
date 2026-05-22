#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod artifacts;
mod engine;
mod error;

pub mod language;
pub mod ner;
pub mod tokenizer;

pub use self::artifacts::{Artifacts, Token};
pub use self::engine::{
    Context, ContextBuilder, ContextBuilderError, Engine, EngineBuilder, EngineBuilderError,
    NlpPreset,
};
pub use self::error::{Error, Result};
pub use self::language::{
    LanguageDetection, LanguagePolicy, LanguageProvenance, LanguageSpan, LinguaLanguageDetector,
    LinguaLanguagePolicy,
};
#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use self::ner::NoopNerBackend;
pub use self::ner::{NerBackend, OrtNerBackend, OrtNerConfig};
pub use self::tokenizer::{HfTokenizer, Tokenizer, UnicodeTokenizer};
