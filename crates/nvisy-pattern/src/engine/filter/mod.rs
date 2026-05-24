//! Per-scan filters and hints: suppression, forced detection,
//! caller-supplied context.
//!
//! All types are part of the public API and configured by the
//! caller on each [`PatternEngine::scan_entities`] invocation.
//!
//! [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities

mod allow_list;
mod context_hint;
mod deny_list;
mod deny_scanner;

use serde::{Deserialize, Serialize};

pub use self::allow_list::AllowList;
pub use self::context_hint::ContextHint;
pub use self::deny_list::{DenyList, DenyRule};
use crate::patterns::RuntimePattern;

/// Per-scan configuration for allow/deny lists and context hints.
///
/// Passed to [`PatternEngine::scan_entities`] to control
/// per-invocation suppression, forced detection, and context-aware
/// confidence boosting without rebuilding the engine.
///
/// All fields default to empty, so `ScanContext::default()` is a
/// no-op context. The type is `Serialize + Deserialize` so an HTTP
/// API can accept a `ScanContext` as JSON request body.
///
/// [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ScanContext {
    /// Values to silently drop from results.
    #[serde(default)]
    pub allow: AllowList,
    /// Values to inject as synthetic matches when found in text.
    #[serde(default)]
    pub deny: DenyList,
    /// Caller-supplied context keywords, optionally scoped per
    /// [`EntityKind`]. The enhancer picks at most one bucket per
    /// match: the entry whose `kind == Some(match.entity_kind)`,
    /// or the first entry with `kind == None` as fallback.
    ///
    /// [`EntityKind`]: nvisy_ontology::entity::EntityKind
    #[serde(default)]
    pub hints: Vec<ContextHint>,
    /// Per-call ad-hoc patterns to scan alongside the engine's
    /// registry-defined ones. Compiled on the hot path on every
    /// scan when non-empty — [#188] tracks adding an LRU cache.
    ///
    /// **Filter bypass**: the engine's [`PatternFilter`] (language,
    /// region, compliance) applies only to registry patterns built
    /// at engine-construction time. Entries in this vec scan
    /// unconditionally — extras are explicit caller intent and the
    /// caller is expected to scope them themselves.
    ///
    /// Compile errors against a malformed extra are silently
    /// dropped during scanning — call
    /// [`PatternEngine::validate_runtime_patterns`] beforehand if
    /// you need to surface them.
    ///
    /// [`PatternFilter`]: super::PatternFilter
    /// [`PatternEngine::validate_runtime_patterns`]: super::PatternEngine::validate_runtime_patterns
    /// [#188]: https://github.com/nvisycom/runtime/issues/188
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_patterns: Vec<RuntimePattern>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scan_context_deserializes_from_empty_object() {
        let ctx: ScanContext = serde_json::from_str("{}").expect("empty object");
        assert!(ctx.allow.is_empty());
        assert!(ctx.deny.is_empty());
        assert!(ctx.hints.is_empty());
    }
}
