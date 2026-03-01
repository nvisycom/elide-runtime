#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod bridge;
mod local;

pub mod prelude;

pub use backend::{OcrBackend, OcrConfig, OcrRegion};
pub use local::LocalOcrBackend;
