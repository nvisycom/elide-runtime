#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod providers;
pub mod streams;

#[doc(hidden)]
pub mod prelude;
