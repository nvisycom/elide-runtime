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
pub use self::engine::{Engine, EngineBuilder, NoLang, NoNer, WithLang, WithNer};
pub use self::error::{Error, Result};
pub use self::language::{
    LanguageDetection, LanguageDetector, LanguageProvenance, LanguageSpan, LinguaLanguageDetector,
};
pub use self::ner::{NerBackend, NoopNerBackend, OrtNerBackend, OrtNerConfig};
pub use self::tokenizer::{HfTokenizer, Tokenizer, UnicodeTokenizer};
