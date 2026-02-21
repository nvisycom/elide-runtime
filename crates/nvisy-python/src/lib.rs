#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod bridge;
pub mod ner;
pub mod ocr;

#[doc(hidden)]
pub mod prelude;
