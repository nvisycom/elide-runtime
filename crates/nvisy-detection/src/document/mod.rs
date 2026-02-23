//! Document-level detection actions.

pub mod dedup;
pub mod manual;

pub use dedup::DeduplicateAction;
pub use manual::{DetectManualAction, DetectManualParams, Exclusion, ManualOutput, is_excluded};
