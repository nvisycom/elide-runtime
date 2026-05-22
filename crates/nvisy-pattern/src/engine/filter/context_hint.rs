//! [`ContextHint`]: one bucket of caller-supplied context keywords
//! (with optional window/boost overrides), optionally scoped to a
//! single [`EntityKind`].
//!
//! Hints are merged into the surrounding-words search performed by
//! the context-aware enhancer. They cannot promote matches whose
//! pattern definitions don't already declare a context rule —
//! patterns must opt into context-aware scoring, and hints layer on
//! top.
//!
//! Per-match resolution: the enhancer picks the [`ContextHint`]
//! whose `kind == Some(match.entity_kind)`, falling back to the
//! entry with `kind == None` (the global bucket) when there is no
//! kind-specific match. At most one bucket applies per match — this
//! avoids "DOB" keywords accidentally boosting an SSN pattern that
//! happens to be context-aware.

use nvisy_ontology::entity::EntityKind;
use serde::{Deserialize, Serialize};

/// Caller-supplied context for this scan, optionally scoped to a
/// single [`EntityKind`].
///
/// `keywords` are appended to whatever each match's pattern-level
/// context rule already declares (within the matching bucket).
/// `window` and `boost` are optional per-call overrides — they only
/// take effect when `keywords` is non-empty.
///
/// Set `kind` to `None` to make this a global fallback bucket,
/// applied when no kind-specific hint matches the entity kind.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ContextHint {
    /// The entity kind this hint applies to, or `None` for a
    /// global fallback bucket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<EntityKind>,
    /// Additional keywords to search for in each matching pattern's
    /// window. Merged (union) with the pattern's own context
    /// keywords.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Per-call window override (bytes before and after the match).
    /// Ignored when `keywords` is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<usize>,
    /// Per-call boost override applied when at least one keyword
    /// (own or hint) is found. Ignored when `keywords` is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boost: Option<f64>,
}

impl ContextHint {
    /// True when this hint has no extra keywords to contribute.
    /// Used by the enhancer to skip merging and override logic on
    /// the hot path.
    pub(crate) fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }
}
