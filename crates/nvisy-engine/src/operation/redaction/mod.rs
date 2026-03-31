//! Redaction operation: policy evaluation + application.
//!
//! Phase 4 of the pipeline. Two steps:
//!
//! 1. **Evaluate**: match entities against policy rules to produce
//!    [`RedactionDecision`]s (in [`evaluate`]).
//! 2. **Apply**: compute replacement text from each decision's strategy
//!    and build codec [`TextRedaction`] instructions (in [`apply`]).

pub(crate) mod apply;
mod evaluate;

pub use self::evaluate::RedactionOp;
