#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod agent;
pub mod audio;
pub mod backend;
pub mod error;
#[doc(hidden)]
pub mod prelude;
