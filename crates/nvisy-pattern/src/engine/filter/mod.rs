//! Per-scan filters: suppression, forced detection, and the context
//! that bundles them.
//!
//! All three types are part of the public API and configured by the
//! caller on each [`PatternEngine::scan_entities`] invocation.
//!
//! [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities

mod allow_list;
mod deny_list;
mod scan_context;

pub use self::allow_list::AllowList;
pub use self::deny_list::{DenyList, DenyRule};
pub use self::scan_context::ScanContext;
