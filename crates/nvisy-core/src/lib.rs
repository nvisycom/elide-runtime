#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod context;
pub mod file;
pub mod llm;
pub mod plan;
pub mod policy;
pub mod service;

pub use self::file::{FileLineage, FileMetadata, RawDocument};
pub use self::service::{Error, ErrorKind, Result};
