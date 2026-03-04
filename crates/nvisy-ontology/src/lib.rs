#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod context;
pub mod entity;
pub mod location;
pub mod policy;
pub mod record;
pub mod specification;

#[doc(hidden)]
pub mod prelude;
