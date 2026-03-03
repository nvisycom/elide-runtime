//! Processing operations: deterministic transformations on content.

mod deduplication;
mod ensemble_fusion;
mod manual_detection;
mod pattern_match;
mod policy_evaluation;
mod redaction;
mod validation;

pub use deduplication::Deduplication;
pub use ensemble_fusion::{Ensemble, FusionStrategy};
pub use manual_detection::{ManualDetection, ManualDetectionParams, Exclusion, ManualOutput, is_excluded};
pub use pattern_match::{PatternDetectionParams, PatternInput, PatternMatch};
pub use policy_evaluation::{EvaluatePolicy, EvaluatePolicyParams};
pub use redaction::{Redaction, RedactionInput, RedactionOutput};
pub use validation::Validation;
