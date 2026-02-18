//! Domain types for pipeline processing.
//!
//! Entity, detection, policy, redaction, and audit types used by pipeline actions.

mod audit;
mod detection;
mod entity;
mod policy;
mod redaction;

pub use audit::{Audit, AuditAction, Auditable, RetentionPolicy, RetentionScope};
pub use detection::{
    Annotation, AnnotationKind, AnnotationLabel, AnnotationScope, ClassificationResult,
    DetectionResult, Sensitivity, SensitivityLevel,
};
pub use entity::{
    AudioLocation, DetectionMethod, Entity, EntitySelector, ImageLocation, ModelInfo, ModelKind,
    TabularLocation, TextLocation, VideoLocation,
};
pub use policy::{
    Policies, Policy, PolicyEvaluation, PolicyRule, RegulationKind, RuleCondition, RuleKind,
};
pub use redaction::{
    AudioRedactionSpec, ImageRedactionSpec, Redactable, Redaction, RedactionSpec,
    RedactionSummary, ReviewDecision, ReviewStatus, TextRedactionSpec,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
