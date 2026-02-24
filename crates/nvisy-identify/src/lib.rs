#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod ontology;
mod layer;
mod pattern;
mod ner;
mod vision;
mod llm;
mod audio;
mod fusion;
mod policy;

pub mod prelude;

// --- Domain types ---
pub use ontology::*;

// --- Layer traits ---
pub use layer::*;

// --- NER backend ---
pub use ner::{NerBackend, NerConfig};

// --- Detection layers ---
pub use pattern::{PatternDetection, PatternDetectionParams};
pub use ner::{NerDetection, NerDetectionParams};
pub use ner::ImageNerDetection;
pub use vision::{FaceBackend, FaceDetection, ObjectBackend, ObjectDetection, OcrDetection};
pub use llm::{LlmDetection, LlmDetectionParams, user_prompt as llm_user_prompt};
pub use audio::TranscriptNerDetection;

// --- Post-detection actions ---
pub use fusion::{DetectManualAction, DetectManualParams, Exclusion, ManualOutput, is_excluded};
pub use fusion::DeduplicateAction;
pub use fusion::{EnsembleMerge, FusionStrategy};

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
