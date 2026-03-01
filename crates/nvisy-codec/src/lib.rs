#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod handler;
pub mod stream;
pub mod document;
pub mod transform;

#[doc(hidden)]
pub mod prelude;
