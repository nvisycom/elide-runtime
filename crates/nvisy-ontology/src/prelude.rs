//! Convenience re-exports for common nvisy-ontology types.

pub use crate::audit::{
    Audit, AuditAction, Auditable, Explainable, Explanation, RetentionPolicy, RetentionScope,
};
pub use crate::detection::{
    Annotation, AnnotationKind, AnnotationLabel, ClassificationResult, Detectable,
    DetectionResult, SensitivityLevel,
};
pub use crate::entity::{
    AudioLocation, BoundingBox, DetectionMethod, Entity, EntityCategory, EntityLocation,
    EntitySelector, ImageLocation, ModelInfo, ModelKind, TabularLocation, TextLocation, TimeSpan,
    VideoLocation,
};
pub use crate::policy::{
    Policy, PolicyEvaluation, PolicyRule, RegulationKind, RuleCondition, RuleKind,
};
pub use crate::redaction::{
    AudioRedactionMethod, AudioRedactionOutput, AudioRedactionSpec, ImageRedactionMethod,
    ImageRedactionOutput, ImageRedactionSpec, Redactable, Redaction, RedactionMethod,
    RedactionOutput, RedactionSpec, RedactionSummary, ReviewDecision, ReviewStatus,
    TextRedactionMethod, TextRedactionOutput, TextRedactionSpec,
};
