#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod detection;
pub(crate) mod graph;
pub mod operation;
pub mod pipeline;
pub mod registry;
pub mod utility;
pub mod workflow;
