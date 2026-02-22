#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub(crate) mod patterns;
pub(crate) mod dictionaries;
pub(crate) mod validators;
mod engine;

pub use engine::{
    PatternEngine, PatternEngineBuilder, PatternEngineError, PatternMatch, DetectionSource,
    AllowList, DenyEntry, DenyList, default_engine,
};
pub use patterns::ContextRule;

pub mod prelude;
