//! Entity detection actions.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! that produces [`Entity`](crate::ontology::entity::Entity) values from
//! document content.

/// Validates detected entities using checksum algorithms (e.g. Luhn).
pub mod checksum;
/// Computes a sensitivity classification for each blob based on detected entities.
pub mod classify;
/// Aho-Corasick dictionary-based entity detection.
pub mod dictionary;
/// Converts user-provided manual annotations into entities.
pub mod manual;
/// AI-powered named-entity recognition (text + image).
pub mod ner;
/// Scans document text with compiled regex patterns to detect PII/PHI entities.
pub mod regex;
/// Column-based rule matching for tabular data.
pub mod tabular;
