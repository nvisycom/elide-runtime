#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod entity;
pub mod extraction;
pub mod health;
pub mod modality;
pub mod primitive;
pub mod recognition;
pub mod redaction;

mod error;
pub use self::error::{Error, ErrorKind, Result};
