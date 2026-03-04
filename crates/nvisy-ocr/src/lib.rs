#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod engine;
pub mod provider;

#[doc(hidden)]
pub mod prelude;

pub use backend::{Backend, ImageFormat, ImageInput, ImageOutput, ImageRegion, RunParams};
pub use engine::Engine;
