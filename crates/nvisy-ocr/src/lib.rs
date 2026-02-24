#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod bridge;
mod parse;

pub use backend::{OcrBackend, OcrConfig};
pub use parse::parse_ocr_entities;
