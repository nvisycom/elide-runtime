#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod handler;
pub mod document;
pub mod element;
pub mod text;
pub mod binary;
pub mod image;
pub mod tabular;
pub mod audio;

#[doc(hidden)]
pub mod prelude;
