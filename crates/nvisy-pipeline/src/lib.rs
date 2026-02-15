//! Pipeline action/provider traits with detection, redaction, and generation actions.
//!
//! This crate consolidates the processing pipeline: the [`Action`] and
//! [`Provider`] traits, entity detection (regex, dictionary, checksum,
//! tabular, manual, NER), policy evaluation, content redaction
//! (text/image/tabular/audio), content generation (OCR, transcription,
//! synthetic data), and audit-trail emission.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// The `Action` trait — the fundamental processing unit in a pipeline.
pub mod action;
/// The `Provider` trait — factory for authenticated client connections.
pub mod provider;
/// Entity detection actions.
pub mod detection;
/// Redaction actions (policy evaluation, apply, audit).
pub mod redaction;
/// Content generation actions (OCR, transcription, synthetic data).
pub mod generation;
#[doc(hidden)]
pub mod prelude;
