#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod engine;

pub mod backend;
pub mod core;
pub mod language;

pub use self::backend::NerBackend;
pub use self::core::{Backend, Context};
pub use self::engine::{Artifacts, Recognizer, RecognizerBuilder, RecognizerBuilderError};
