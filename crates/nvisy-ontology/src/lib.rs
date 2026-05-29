#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;

pub mod context;
pub mod document;
pub mod entity;
pub mod modality;
pub mod policy;
pub mod primitive;
pub mod provenance;

pub use self::error::{Error, Result};
