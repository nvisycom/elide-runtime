#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// The `Action` trait — the fundamental processing unit in a pipeline.
pub mod action;
/// The `Provider` trait — factory for authenticated client connections.
pub mod provider;
/// Entity detection actions.
pub mod detection;
/// Content generation actions (OCR, transcription, synthetic data).
pub mod generation;
/// Domain types: entity, detection, policy, and redaction.
pub mod ontology;
/// Redaction actions (policy evaluation, apply, audit).
pub mod redaction;
#[doc(hidden)]
pub mod prelude;
