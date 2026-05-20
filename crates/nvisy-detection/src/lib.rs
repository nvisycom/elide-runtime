#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod engine;
mod error;
mod extension;
mod recognizer;

pub use self::engine::{
    DetectionContext, DetectionContextBuilder, DetectionContextBuilderError, DetectionEngine,
    DetectionEngineBuilder, DetectionEngineBuilderError,
};
pub use self::error::{Error, Result};
pub use self::extension::RebaseEntities;
pub use self::recognizer::{LlmRecognizer, NerRecognizer, PatternRecognizer, Recognizer};
