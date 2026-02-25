//! PaddleOCR / OCR backend integration.
//!
//! Re-exports the OCR backend trait, configuration, entity parsing, and
//! the [`PythonBridge`] implementation.

mod backend;
mod bridge;
mod parse;

pub use backend::{OcrBackend, OcrConfig};
pub use parse::parse_ocr_entities;
