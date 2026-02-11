//! PII/PHI detection actions for the nvisy pipeline.
//!
//! This crate provides the detection, classification, policy evaluation,
//! redaction, and audit-trail stages used by the nvisy runtime. It also
//! ships a built-in set of regex patterns compiled from `assets/patterns.json`.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Pipeline actions for detection, classification, policy, redaction, and audit.
pub mod actions;
/// Built-in dictionary data for name and term matching.
pub mod dictionaries;
/// Built-in regex pattern definitions and validation helpers.
pub mod patterns;

#[doc(hidden)]
pub mod prelude;
