//! Policy types, rules, and governance structures.

mod evaluation;
mod regulation;
mod retention;
mod rule;
mod selector;
mod strategy;
mod summary;
mod types;

pub use evaluation::PolicyEvaluation;
pub use regulation::RegulationKind;
pub use retention::{RetentionPolicy, RetentionScope};
pub use rule::{PolicyRule, RuleAction, RuleCondition};
pub use selector::EntitySelector;
pub use strategy::{
    AudioRedactionStrategy, ImageRedactionStrategy, RedactionStrategy, TextRedactionStrategy,
    DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_MASK_CHAR, DEFAULT_PIXELATE_BLOCK_SIZE,
};
pub use summary::RedactionSummary;
pub use types::{Policies, Policy};
