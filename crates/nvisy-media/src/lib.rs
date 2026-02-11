#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Image rendering primitives (blur, block overlay).
pub mod render;
/// Pipeline actions for applying redactions to media.
pub mod actions;

#[doc(hidden)]
pub mod prelude;
