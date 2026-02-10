#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod data;
pub mod datatypes;
pub mod documents;
pub mod errors;
pub mod plugin;
pub mod registry;
pub mod traits;
pub mod types;
