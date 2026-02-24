#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod bridge;
mod parse;

pub use backend::{TranscribeBackend, TranscribeConfig};
pub use parse::parse_transcribe_entities;
