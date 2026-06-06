#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
mod extraction;

pub use self::extraction::{SttExtractor, Transcription};
