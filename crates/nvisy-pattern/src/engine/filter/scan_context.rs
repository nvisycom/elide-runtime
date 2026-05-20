//! [`ScanContext`]: per-scan allow/deny list configuration.

use super::allow_list::AllowList;
use super::deny_list::DenyList;

/// Per-scan configuration for allow and deny lists.
///
/// Passed to [`PatternEngine::scan_entities`]
/// to control per-invocation suppression and forced detection without
/// rebuilding the engine.
///
/// Both fields default to empty, so `ScanContext::default()` is a no-op
/// context.
///
/// [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities
#[derive(Debug, Default)]
pub struct ScanContext {
    /// Values to silently drop from results.
    pub allow: AllowList,
    /// Values to inject as synthetic matches when found in text.
    pub deny: DenyList,
}
