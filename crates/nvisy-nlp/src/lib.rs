#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod engine;
mod error;

pub mod language;
pub mod ner;

pub use self::engine::{
    Artifacts, NlpContext, NlpContextBuilder, NlpContextBuilderError, NlpEngine, NlpEngineBuilder,
    NlpEngineBuilderError,
};
pub use self::error::{Error, Result};
