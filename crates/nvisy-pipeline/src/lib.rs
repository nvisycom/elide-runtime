#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// The `Provider` trait — factory for authenticated client connections.
pub mod provider;
/// Streaming source and target traits for pipeline I/O.
pub mod stream;
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
