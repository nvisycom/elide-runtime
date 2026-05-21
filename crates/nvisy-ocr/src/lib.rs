#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod engine;
pub mod provider;

pub use self::backend::{Backend, ImageFormat, ImageInput, ImageOutput, RunParams};
pub use self::engine::{OcrEngine, OcrProvider};
