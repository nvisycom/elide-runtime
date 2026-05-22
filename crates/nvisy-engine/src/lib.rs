#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod deduplication;
pub mod detection;
pub mod envelope;
pub mod extraction;
pub mod ingestion;
pub mod pipeline;
pub mod redaction;
pub mod validation;
