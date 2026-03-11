//! Processing operations: deterministic transformations on detected entities
//! and document content.
//!
//! These operations run without external model calls and are fully
//! reproducible given the same inputs.
//!
//! | Operation           | Description                                          |
//! |---------------------|------------------------------------------------------|
//! | [`PatternMatch`]    | Regex + dictionary entity detection across modalities|
//! | [`ManualDetection`] | Converts user annotations into entities/exclusions   |
//! | [`Deduplication`]   | Merges overlapping duplicate entities                 |
//! | [`Ensemble`]        | Fuses multi-detector entities with confidence fusion  |
//! | [`EvaluatePolicy`]  | Maps entities to redaction decisions via policy rules |
//! | [`Redaction`]       | Applies redaction instructions to document content   |
//! | [`Validation`]      | Validates content integrity or conformance           |

mod deduplication;
mod ensemble_fusion;
mod manual_detection;
mod pattern_match;
mod policy_evaluation;
mod redaction;
mod validation;

pub use deduplication::Deduplication;
pub use ensemble_fusion::{Ensemble, FusionStrategy};
pub use manual_detection::{
    Exclusion, ManualDetection, ManualDetectionParams, ManualOutput, is_excluded,
};
pub use pattern_match::{PatternDetectionParams, PatternInput, PatternMatch};
pub use policy_evaluation::{EvaluatePolicy, EvaluatePolicyParams};
pub use redaction::{Redaction, RedactionInput, RedactionOutput};
pub use validation::Validation;
