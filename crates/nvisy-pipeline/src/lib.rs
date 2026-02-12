//! Pipeline action/provider traits with detection and redaction actions.
//!
//! This crate consolidates the processing pipeline: the [`Action`] and
//! [`Provider`] traits, all detection actions (regex, dictionary, checksum,
//! tabular, manual), policy evaluation, text/image/tabular/PDF/audio
//! redaction, and audit-trail emission.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// The `Action` trait — the fundamental processing unit in a pipeline.
pub mod action;
/// The `Provider` trait — factory for authenticated client connections.
pub mod provider;
/// Pipeline actions for detection, redaction, policy, and audit.
pub mod actions;
/// Image rendering primitives for redaction overlays.
#[cfg(feature = "image-redaction")]
pub mod render;

#[doc(hidden)]
pub mod prelude;
