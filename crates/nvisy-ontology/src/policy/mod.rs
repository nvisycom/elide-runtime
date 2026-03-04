//! Policy types, redaction specifications, and governance structures.

mod evaluation;
mod regulation;
mod retention;
mod rule;
mod summary;
mod types;

pub use evaluation::PolicyEvaluation;
pub use regulation::RegulationKind;
pub use retention::{RetentionPolicy, RetentionScope};
pub use rule::{PolicyRule, RuleCondition, RuleKind};
pub use summary::RedactionSummary;
pub use types::{Policies, Policy};

// Re-export data types from sibling modules for convenience.
pub use crate::record::Redaction;
pub use crate::record::{ReviewDecision, ReviewStatus};
pub use crate::specification::{
    AudioRedactionInput, ImageRedactionInput, RedactionInput, TextRedactionInput,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
