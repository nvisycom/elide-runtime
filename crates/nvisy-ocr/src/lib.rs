#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod bridge;
mod local;
mod parse;

pub mod prelude;

pub use backend::{OcrBackend, OcrConfig, OcrRegion};
pub use local::LocalOcrBackend;
pub use parse::parse_ocr_entities;
