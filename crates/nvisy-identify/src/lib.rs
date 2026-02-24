#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod ontology;
mod layer;
mod ner;
mod text;
mod image;
mod action;
mod policy;

pub mod prelude;

// --- Domain types ---
pub use ontology::*;

// --- Layer traits ---
pub use layer::*;

// --- NER backend ---
pub use ner::{NerBackend, NerConfig};

// --- Detection layers ---
pub use text::{PatternDetection, PatternDetectionParams};
pub use text::{NerDetection, NerDetectionParams};
pub use image::{ImageNerDetection, FaceBackend, FaceDetection, ObjectBackend, ObjectDetection};

// --- Post-detection actions ---
pub use action::{DetectManualAction, DetectManualParams, Exclusion, ManualOutput, is_excluded};
pub use action::DeduplicateAction;

// --- Policy & governance ---
pub use policy::{
    Policy, Policies, PolicyRule, RuleKind, RuleCondition,
    RedactionInput, TextRedactionInput, ImageRedactionInput, AudioRedactionInput,
    Redaction, PolicyEvaluation, RedactionSummary,
    RegulationKind, RetentionPolicy, RetentionScope,
    ReviewDecision, ReviewStatus,
    Audit, AuditAction,
    EvaluatePolicyAction, EvaluatePolicyParams,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
