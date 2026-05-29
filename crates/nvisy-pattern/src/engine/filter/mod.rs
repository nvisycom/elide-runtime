//! Per-scan filters and hints: suppression, forced detection,
//! caller-supplied context.
//!
//! All types are part of the public API and configured by the
//! caller on each [`PatternEngine::scan_text`] invocation.
//!
//! [`PatternEngine::scan_text`]: super::PatternEngine::scan_text

mod allow_list;
mod context_hint;
mod deny_list;
mod deny_scanner;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::allow_list::AllowList;
pub use self::context_hint::ContextHint;
pub use self::deny_list::{DenyList, DenyRule};
use crate::patterns::RuntimePattern;

/// Per-scan configuration for allow/deny lists, context hints, and
/// caller-supplied ad-hoc patterns.
///
/// Passed to [`PatternEngine::scan_text`] to control
/// per-invocation suppression, forced detection, and context-aware
/// confidence boosting without rebuilding the engine.
///
/// All fields default to empty, so `PatternContext::default()` is a
/// no-op context. The type is `Serialize + Deserialize` so an HTTP
/// API can accept a `PatternContext` as JSON request body.
///
/// [`PatternEngine::scan_text`]: super::PatternEngine::scan_text
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PatternContext {
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
    /// [`PatternEngine::validate_patterns`] beforehand if
    /// you need to surface them.
    ///
    /// [`PatternFilter`]: super::PatternFilter
    /// [`PatternEngine::validate_patterns`]: super::PatternEngine::validate_patterns
    /// [#188]: https://github.com/nvisycom/runtime/issues/188
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_patterns: Vec<RuntimePattern>,
    /// Correlation UUID propagated through the tracing span for this
    /// scan. Not used for detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
}
