#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod datatypes;
pub mod error;
pub mod fs;
pub mod io;
pub mod path;
pub mod registry;

#[doc(hidden)]
pub mod prelude;
