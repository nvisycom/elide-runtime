#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// File-format loaders.
pub mod loaders;

#[doc(hidden)]
pub mod prelude;
