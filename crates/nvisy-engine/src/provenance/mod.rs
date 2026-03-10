//! Provenance: audit trails, redaction records, and per-file processing logs.
//!
//! This module is the single source of truth for all pipeline audit and
//! decision types. It combines two concerns:
//!
//! - **Execution logs** ([`Audit`], [`AuditEntry`]) — when operations
//!   ran, how long they took, what models were used, token counts, etc.
//!
//! - **Redaction records** ([`RedactionDecision`], [`RedactionRecord`],
//!   [`PolicyEvaluation`], [`RedactionMap`]) — what was redacted, why, and
//!   human-review status.
//!
//! Together these form a complete audit trail for compliance and review.

mod audit;
mod entry;
mod kind;

mod action;
mod record;

pub use action::{
    InferenceAction, InferenceActionBuilder, LifecycleAction, LifecycleActionBuilder,
    ProcessingAction, ProcessingActionBuilder,
};
pub use audit::Audit;
pub use entry::{AuditEntry, AuditEntryBuilder, AuditEntryBuilderError, AuditEntryStatus};
pub use kind::{AuditEntryKind, InferenceKind, LifecycleKind, ProcessingKind};
pub use record::{
    PolicyEvaluation, RedactionDecision, RedactionMap, RedactionMapEntry, RedactionRecord,
    ReviewDecision, ReviewStatus,
};
