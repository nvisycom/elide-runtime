//! Detection and redaction domain types.
//!
//! Types in this module represent the core ontology of the nvisy pipeline:
//! entities (detected sensitive data), redactions (how entities are masked),
//! and audit records (immutable event log).

pub mod audit;
pub mod entity;
pub mod redaction;

pub use audit::{Audit, AuditAction};
pub use entity::{
    BoundingBox, DetectionMethod, Entity, EntityCategory, EntityLocation,
};
pub use redaction::{Redaction, RedactionMethod};
