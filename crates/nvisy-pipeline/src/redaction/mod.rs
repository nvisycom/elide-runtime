//! Redaction actions.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! that evaluates, applies, or records redaction decisions.

mod apply;
mod emit_audit;
mod evaluate_policy;

pub use apply::{
    ApplyRedactionAction, ApplyRedactionInput, ApplyRedactionOutput, ApplyRedactionParams,
};
pub use emit_audit::{EmitAuditAction, EmitAuditParams};
pub use evaluate_policy::{EvaluatePolicyAction, EvaluatePolicyParams};
