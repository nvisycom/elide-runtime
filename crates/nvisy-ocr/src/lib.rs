#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod core;
pub mod engine;

pub use self::backend::OcrBackend;
pub use self::core::{Backend, Context, ImageFormat, ImageInput, ImageOutput};
pub use self::engine::Extractor;
