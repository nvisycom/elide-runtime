#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub(crate) mod dictionaries;
pub(crate) mod engine;
pub(crate) mod patterns;
pub(crate) mod validators;

pub use self::engine::{
    PatternEngine, PatternEngineBuilder, PatternEngineError, PatternFilter, filter,
};
