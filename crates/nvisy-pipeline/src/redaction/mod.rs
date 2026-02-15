//! Redaction actions.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! that evaluates, applies, or records redaction decisions.

/// Applies pending redactions to document content (text, image, tabular, audio).
pub mod apply;
/// Image rendering primitives for redaction overlays.
#[cfg(feature = "image-redaction")]
pub mod render;
/// Emits audit trail records for every applied redaction.
pub mod emit_audit;
/// Evaluates policy rules against detected entities and produces redaction instructions.
pub mod evaluate_policy;
