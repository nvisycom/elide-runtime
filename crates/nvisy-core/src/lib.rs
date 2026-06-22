#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// context is awaiting its redesign pass on elide types; gated out so
// policy + service + schema can move forward independently.
// pub mod context;
pub mod policy;
pub mod schema;
pub mod service;

pub use self::service::{Error, ErrorKind, Result};
