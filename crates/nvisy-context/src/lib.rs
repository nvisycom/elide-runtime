#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod enhancer;
mod matcher;
mod rule;
mod tokens;
mod wrapper;

pub use self::enhancer::Enhancer;
pub use self::matcher::{KeywordMatcher, LemmaMatcher, SubstringMatcher};
pub use self::rule::{BoostRule, DEFAULT_BOOST, DEFAULT_PREFIX_WORDS, DEFAULT_SUFFIX_WORDS};
pub use self::tokens::{Token, Tokens};
pub use self::wrapper::Boosting;
