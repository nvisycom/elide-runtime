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
//! to set up. The [`RedactorDefaults`] config section only supplies
//! fallback values for workflow [`Redaction`] fields that aren't
//! explicitly set.
//!
//! [`AuditEntry`]: nvisy_ontology::provenance::AuditEntry

mod apply;
mod defaults;
mod evaluate;
mod tts;
mod workflow;

pub use self::defaults::RedactorDefaults;
pub use self::evaluate::Redactor;
pub use self::tts::RedactorTtsConfig;
pub use self::workflow::Redaction;
