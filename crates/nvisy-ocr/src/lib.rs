#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod engine;
pub mod provider;

#[doc(hidden)]
pub mod prelude;

pub use self::backend::{
    Backend, Block, BlockKind, ImageFormat, ImageInput, ImageOutput, Line, Page, RunParams, Word,
};
pub use self::engine::{OcrEngine, OcrProvider};
