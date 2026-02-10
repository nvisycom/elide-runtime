//! Pipeline actions for the detection and redaction workflow.
//!
//! Each sub-module exposes a single [`Action`](nvisy_core::traits::action::Action)
//! implementation that can be wired into an nvisy execution plan.

/// Applies pending redactions to document content.
pub mod apply_redaction;
/// Computes a sensitivity classification for each blob based on detected entities.
pub mod classify;
/// Validates detected entities using checksum algorithms (e.g. Luhn).
pub mod detect_checksum;
/// Scans document text with compiled regex patterns to detect PII/PHI entities.
pub mod detect_regex;
/// Emits audit trail records for every applied redaction.
pub mod emit_audit;
/// Evaluates policy rules against detected entities and produces redaction instructions.
pub mod evaluate_policy;
