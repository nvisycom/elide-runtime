#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod fs;
pub mod io;
pub mod math;
pub mod path;

mod error;
pub use error::{Error, ErrorKind, Result};

#[doc(hidden)]
pub mod prelude;
