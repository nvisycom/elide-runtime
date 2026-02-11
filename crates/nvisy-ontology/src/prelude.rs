//! Convenience re-exports for common nvisy-ontology types.

pub use crate::ontology::{
    Audit, AuditAction, BoundingBox, DetectionMethod, Entity, EntityCategory,
    EntityLocation, Redaction, RedactionMethod,
};
pub use crate::redaction::{
    EntityRedactionRule, ManualAnnotation, Policy, PolicyRule, RedactionContext,
};
