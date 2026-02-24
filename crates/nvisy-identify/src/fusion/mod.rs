//! Post-detection entity merging, deduplication, and manual annotations.

pub mod dedup;
pub mod ensemble;
pub mod manual;

pub use dedup::DeduplicateAction;
pub use ensemble::{EnsembleMerge, FusionStrategy};
pub use manual::{DetectManualAction, DetectManualParams, Exclusion, ManualOutput, is_excluded};
