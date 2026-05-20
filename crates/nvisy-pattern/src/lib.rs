#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod dictionaries;
pub mod engine;
pub mod patterns;
pub(crate) mod validators;

pub use self::engine::{
    AllowList, DenyList, DenyRule, PatternEngine, PatternEngineBuilder, PatternEngineError,
    ScanContext,
};
