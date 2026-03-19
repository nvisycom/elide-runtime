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
//! | [`Fusion`]          | Deduplication + ensemble confidence fusion            |
//! | [`EvaluatePolicy`]  | Maps entities to redaction decisions via policy rules |
//! | [`Redaction`]       | Applies redaction instructions to document content   |
//! | [`Validation`]      | Verifies redacted content does not leak originals    |

mod fusion;
mod manual_detection;
mod pattern_match;
mod policy_evaluation;
mod redaction;
mod validation;

pub use self::fusion::{Fusion, FusionParams, FusionStrategy};
pub use self::manual_detection::{
    Exclusion, ManualDetection, ManualDetectionParams, ManualOutput, is_excluded,
};
pub use self::pattern_match::{PatternDetectionParams, PatternMatch};
pub use self::policy_evaluation::{EvaluatePolicy, EvaluatePolicyParams};
pub use self::redaction::{Redaction, RedactionInput, RedactionOutput};
pub use self::validation::{LeakedValue, Validation, ValidationInput, ValidationOutput};
