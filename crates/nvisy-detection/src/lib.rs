#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod engine;
mod error;
mod extension;
mod recognizer;

pub use nvisy_pattern::PatternFilter;

pub use self::engine::{
    Detection, DetectionContext, DetectionContextBuilder, DetectionContextBuilderError,
    DetectionEngine, DetectionEngineBuilder, DetectionEngineBuilderError,
};
pub use self::error::{Error, Result};
pub use self::extension::RebaseEntities;
pub use self::recognizer::{
    DetectionParams, LlmDetection, LlmRecognizer, NerDetection, NerRecognizer, PatternDetection,
    PatternRecognizer, Recognizer,
};
