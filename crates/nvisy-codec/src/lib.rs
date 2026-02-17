#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod handler;
pub mod document;
pub mod render;

#[doc(hidden)]
pub mod prelude;
