#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod detect;
pub mod document;
pub mod handler;
pub mod transform;

#[doc(hidden)]
pub mod prelude;
