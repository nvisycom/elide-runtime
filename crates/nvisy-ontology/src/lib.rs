#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod context;
pub mod entity;
pub mod policy;
pub mod record;

#[doc(hidden)]
pub mod prelude;
