#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod artifacts;
mod engine;
mod error;

pub mod language;
pub mod ner;
pub mod tokenizer;

pub use self::artifacts::{NlpArtifacts, Token};
pub use self::engine::{NlpEngine, NlpEngineBuilder};
pub use self::error::NlpError;
pub use self::language::{
    LanguageDetection, LanguageDetector, LanguageSpan, LinguaLanguageDetector,
};
pub use self::ner::{NerBackend, NoopNerBackend, OrtNerBackend, OrtNerConfig};
pub use self::tokenizer::{HfTokenizer, Tokenizer, UnicodeTokenizer};
