#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Generic sorted registry backing both pattern and dictionary collections.
pub mod registry;
/// Detection patterns loaded from JSON definition files.
pub mod patterns;
/// Named term dictionaries loaded from text and CSV files.
pub mod dictionaries;
/// Post-match validators for reducing false positives.
pub mod validators;

#[doc(hidden)]
pub mod prelude;
