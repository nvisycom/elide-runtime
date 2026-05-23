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
    Artifacts, Context, ContextBuilder, ContextBuilderError, NlpEngine, NlpEngineBuilder,
    NlpEngineBuilderError,
};
pub use self::error::{Error, Result};
