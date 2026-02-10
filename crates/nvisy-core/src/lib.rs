#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod datatypes;
pub mod error;
pub mod ontology;
pub mod redaction;
pub mod registry;

#[doc(hidden)]
pub mod prelude;
