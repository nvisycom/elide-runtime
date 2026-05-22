#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod detection;
pub mod extraction;
pub mod ingestion;
pub mod operation;
pub mod pipeline;
pub mod redaction;
pub mod registry;
