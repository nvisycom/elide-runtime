//! Content generation actions.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! that generates derived content (text, entities, or replacement values)
//! from documents.

/// OCR text extraction from image documents.
#[cfg(feature = "image-redaction")]
pub mod ocr;
/// Synthetic replacement value generation for Synthesize redactions.
pub mod synthetic;
/// Speech-to-text transcription from audio documents.
#[cfg(feature = "audio-redaction")]
pub mod transcribe;
