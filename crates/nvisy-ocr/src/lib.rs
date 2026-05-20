#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod http;
pub mod ocr;

// Hoist the OCR types to the crate root because everything in this
// crate is OCR-related. Mirrors the legacy `nvisy_ocr::ocr::*`
// path so consumers can transition smoothly.
pub use self::ocr::*;
