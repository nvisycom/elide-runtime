#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod context;
pub mod policy;
pub mod schema;
pub mod service;
pub mod source;

pub use self::source::ContentSource;

pub use self::service::{Error, ErrorKind, Result};
