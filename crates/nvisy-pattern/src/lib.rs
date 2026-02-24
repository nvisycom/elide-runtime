#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub(crate) mod dictionaries;
mod engine;
pub(crate) mod patterns;
pub(crate) mod validators;

pub use engine::{
    AllowList, DenyEntry, DenyList, DetectionSource, PatternEngine, PatternEngineBuilder,
    PatternEngineError, PatternMatch, default_engine,
};
pub use patterns::ContextRule;

#[doc(hidden)]
pub mod prelude;
