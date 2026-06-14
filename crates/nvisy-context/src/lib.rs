#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod declaration;
mod enhancer;
mod matcher;
mod registry;
mod tokens;

pub use self::declaration::Context;
pub use self::enhancer::{ContextEnhancer, ContextEnhancerBuilder, ContextEnhancerBuilderError};
pub use self::matcher::{KeywordMatcher, LemmaMatcher, SubstringMatcher};
pub use self::registry::ContextRegistry;
pub use self::tokens::{Token, Tokens};
