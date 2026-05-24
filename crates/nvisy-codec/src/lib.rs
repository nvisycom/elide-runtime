#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod document;
pub mod handler;

pub use self::document::{ContentHandle, Located, LocationStream, Span};
