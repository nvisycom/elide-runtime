#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod document;
pub mod handler;
pub mod transform;

pub use self::document::{Document, Span, SpanStream};
