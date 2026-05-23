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
#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use self::ner::NoopNerBackend;
pub use self::ner::{NerBackend, OrtNerBackend, OrtNerConfig, id_to_label_from_config_json};
#[cfg(feature = "preset-download")]
#[cfg_attr(docsrs, doc(cfg(feature = "preset-download")))]
pub use self::preset::downloader::{
    DownloadStage, Downloader, NoopReporter, ProgressReporter, TracingReporter,
};
pub use self::preset::{LabelMapEntry, NlpPreset, PresetManifest};
pub use self::tokenizer::{HfTokenizer, Tokenizer, UnicodeTokenizer};
