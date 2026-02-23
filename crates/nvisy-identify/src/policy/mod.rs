//! Policy types, redaction specifications, and governance structures.

mod audit;
mod evaluation;
mod evaluate;
mod types;
mod regulation;
mod retention;
mod rule;
mod summary;

pub use audit::{Audit, AuditAction};
pub use evaluation::PolicyEvaluation;
pub use evaluate::{EvaluatePolicyAction, EvaluatePolicyParams};
pub use types::{Policies, Policy};
pub use regulation::RegulationKind;
pub use retention::{RetentionPolicy, RetentionScope};
pub use rule::{PolicyRule, RuleCondition, RuleKind};
pub use summary::RedactionSummary;

// Re-export data types from nvisy-ontology
pub use nvisy_ontology::record::Redaction;
pub use nvisy_ontology::record::{ReviewDecision, ReviewStatus};
pub use nvisy_ontology::spec::{
    AudioRedactionInput, ImageRedactionInput, RedactionInput, TextRedactionInput,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
