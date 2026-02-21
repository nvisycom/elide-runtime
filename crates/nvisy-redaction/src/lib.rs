#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod text;
mod image;
mod audio;
mod document;

mod audit;
mod evaluation;
mod policy;
mod record;
mod regulation;
mod retention;
mod review;
mod rule;
mod spec;

pub use document::{ApplyRedactionAction, ApplyRedactionInput, ApplyRedactionOutput};
pub use audit::{Audit, AuditAction};
pub use text::{EvaluatePolicyAction, EvaluatePolicyParams};
pub use evaluation::PolicyEvaluation;
pub use policy::{Policies, Policy};
pub use record::Redaction;
pub use regulation::RegulationKind;
pub use retention::{RetentionPolicy, RetentionScope};
pub use review::{ReviewDecision, ReviewStatus};
pub use rule::{PolicyRule, RuleCondition, RuleKind};
pub use spec::{
    AudioRedactionSpec, ImageRedactionSpec, RedactionSpec, TextRedactionSpec,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
pub use document::RedactionSummary;
