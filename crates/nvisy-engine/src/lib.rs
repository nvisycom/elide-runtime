#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub(crate) mod graph;
pub mod operation;
pub mod pipeline;
pub mod registry;
pub mod utility;
