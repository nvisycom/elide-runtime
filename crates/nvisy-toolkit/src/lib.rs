#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod deduplication;
pub mod detection;
pub mod extraction;
pub mod ingestion;
pub mod redaction;
