//! Backend-shaped OCR output: [`RawOcrBlock`] and its parts.
//!
//! Pre-normalization shapes emitted by an [`OcrBackend`].
//! Consumers wrap each block into the per-document block shape
//! they need; this crate stays free of any orchestrator-level
//! dependency.
//!
//! [`OcrBackend`]: crate::backend::OcrBackend

mod raw_block;

pub use self::raw_block::{OcrBlockKind, OcrSpan, RawOcrBlock};
