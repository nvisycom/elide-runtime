//! Pipeline actions for the detection and redaction workflow.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! implementation that can be wired into an nvisy execution plan.

/// Applies pending redactions to document content.
pub mod apply_redaction;
/// Computes a sensitivity classification for each blob based on detected entities.
pub mod classify;
/// Validates detected entities using checksum algorithms (e.g. Luhn).
pub mod detect_checksum;
/// Aho-Corasick dictionary-based entity detection.
pub mod detect_dictionary;
/// Converts user-provided manual annotations into entities.
pub mod detect_manual;
/// Scans document text with compiled regex patterns to detect PII/PHI entities.
pub mod detect_regex;
/// Column-based rule matching for tabular data.
pub mod detect_tabular;
/// Emits audit trail records for every applied redaction.
pub mod emit_audit;
/// Evaluates policy rules against detected entities and produces redaction instructions.
pub mod evaluate_policy;
/// Applies image redactions (blur, block) to image artifacts.
#[cfg(feature = "image-redaction")]
pub mod apply_image_redaction;
/// Applies redactions to tabular data cells.
pub mod apply_tabular_redaction;
/// Reassembles redacted content into PDF files.
#[cfg(feature = "pdf-redaction")]
pub mod apply_pdf_redaction;
/// Placeholder for audio redaction.
pub mod apply_audio_redaction;
