#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod context;
pub mod entity;
pub mod math;

mod error;
pub use self::error::{Error, Result};

pub mod policy;
pub mod provenance;
pub mod workflow;
