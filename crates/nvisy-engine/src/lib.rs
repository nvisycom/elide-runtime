#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod apply;

pub mod compiler;
pub mod engine;

pub use apply::{ApplyRedactionAction, ApplyRedactionInput, ApplyRedactionOutput};
pub use engine::DefaultEngine;

#[doc(hidden)]
pub mod prelude;
