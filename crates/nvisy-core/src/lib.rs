#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod content;
pub mod media;

mod error;
pub use self::error::{Error, ErrorKind, Result};
