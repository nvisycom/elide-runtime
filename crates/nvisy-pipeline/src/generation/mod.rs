//! Content generation actions.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! that generates derived content (text, entities, or replacement values)
//! from documents.

mod ocr;
mod synthetic;
mod transcribe;

pub use ocr::{
    GenerateOcrAction, GenerateOcrInput, GenerateOcrOutput, GenerateOcrParams,
    OcrBackend, OcrConfig, parse_ocr_entities,
};
pub use synthetic::{GenerateSyntheticAction, GenerateSyntheticInput, GenerateSyntheticParams};
pub use transcribe::{
    GenerateTranscribeAction, GenerateTranscribeInput, GenerateTranscribeOutput,
    GenerateTranscribeParams,
};
