#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Built-in regex pattern definitions and validation helpers.
pub mod patterns;
/// Built-in dictionary data for entity matching.
pub mod dictionaries;

#[doc(hidden)]
pub mod prelude;
