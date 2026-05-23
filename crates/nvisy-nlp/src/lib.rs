#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod engine;
mod error;

pub mod language;
pub mod ner;
pub mod preset;
pub mod tokenizer;

pub use self::engine::{
    Artifacts, Context, ContextBuilder, ContextBuilderError, Engine, EngineBuilder,
    EngineBuilderError, Token,
};
pub use self::error::{Error, Result};
pub use self::language::{
    LanguageDetection, LanguagePolicy, LanguageProvenance, LanguageSpan, LinguaLanguageDetector,
    LinguaLanguagePolicy,
};
pub use self::ner::{NerBackend, NoopNerBackend};
#[cfg(feature = "gliner")]
#[cfg_attr(docsrs, doc(cfg(feature = "gliner")))]
pub use self::ner::{GlinerBackend, GlinerConfig, GlinerMode};
#[cfg(feature = "onnx")]
#[cfg_attr(docsrs, doc(cfg(feature = "onnx")))]
pub use self::ner::{OrtNerBackend, OrtNerConfig, id_to_label_from_config_json};
pub use self::preset::{BackendConfig, LabelMapEntry, NlpPreset, PresetManifest};
#[cfg(feature = "onnx")]
#[cfg_attr(docsrs, doc(cfg(feature = "onnx")))]
pub use self::tokenizer::HfTokenizer;
pub use self::tokenizer::{Tokenizer, UnicodeTokenizer};
