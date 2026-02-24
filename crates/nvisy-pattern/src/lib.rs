#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub(crate) mod dictionaries;
pub mod engine;
pub(crate) mod patterns;
pub(crate) mod validators;

pub use engine::{DetectionSource, PatternEngine, PatternEngineBuilder, PatternMatch};
pub use patterns::ContextRule;

#[doc(hidden)]
pub mod prelude;
