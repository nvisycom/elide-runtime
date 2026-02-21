#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Entity detection actions.
pub mod detection;
/// Content generation actions (OCR, transcription, synthetic data).
pub mod generation;
/// Domain types: entity and detection result.
pub mod ontology;
/// Redaction actions, types, and policy evaluation.
pub mod redaction;

#[doc(hidden)]
pub mod prelude;
