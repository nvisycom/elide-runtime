#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod entity;
pub mod error;
pub mod fs;
pub mod io;
pub mod math;
pub mod path;
#[doc(hidden)]
pub mod prelude;
