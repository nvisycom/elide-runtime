//! Redaction: policy evaluation + multimodal application.
//!
//! Phase 4 of the pipeline. Two steps:
//!
//! 1. **Evaluate**: match entities against policy rules to produce
//!    [`AuditEntry`]s.
//! 2. **Apply**: build per-modality codec instructions (text, image,
//!    audio) from decisions and apply them to the document, writing
//!    replacement values into audit records.
//!
//! Unlike extraction and detection, redaction has no expensive
//! per-run construction — there's no model to load or HTTP client
//! to set up. The [`RedactionSection`] config supplies deployment-wide
//! fallback values for workflow [`Redaction`] fields that aren't
//! explicitly set.
//!
//! [`AuditEntry`]: nvisy_ontology::provenance::AuditEntry

mod apply;
mod evaluate;
mod section;
mod strategy;
mod workflow;

pub use self::evaluate::{ApplyRedactions, Redactor};
pub use self::section::RedactionSection;
pub use self::workflow::Redaction;
