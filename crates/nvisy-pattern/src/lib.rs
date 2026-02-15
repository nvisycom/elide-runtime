//! Built-in regex patterns and dictionaries for PII/PHI detection.
//!
//! This crate provides the embedded pattern definitions (compiled from
//! `assets/patterns.json`) and dictionary data (first names, last names,
//! medical terms) used by the nvisy pipeline's detection actions.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// Built-in regex pattern definitions and validation helpers.
pub mod patterns;
/// Built-in dictionary data for name and term matching.
pub mod dictionaries;

#[doc(hidden)]
pub mod prelude;
