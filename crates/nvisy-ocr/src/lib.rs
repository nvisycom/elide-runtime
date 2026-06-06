#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod core;
pub mod engine;

pub use self::backend::NoopBackend;
#[cfg(feature = "bento")]
pub use self::backend::{BentoBackend, BentoParams};
pub use self::core::{Backend, Context, ImageInput};
pub use self::engine::Extractor;
