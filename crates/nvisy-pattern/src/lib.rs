#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod dictionaries;
pub mod engine;
pub(crate) mod patterns;
pub(crate) mod validators;

pub use self::dictionaries::{DictionaryLoadError, DictionaryRegistry};
pub use self::engine::{PatternEngine, PatternEngineBuilder, RawMatch};
pub use self::patterns::ContextRule;

#[doc(hidden)]
pub mod prelude;
