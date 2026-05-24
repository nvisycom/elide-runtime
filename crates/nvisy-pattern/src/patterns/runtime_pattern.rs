//! Runtime-constructed [`RuntimePattern`] for per-call [`ScanContext`] extras.
//!
//! Built programmatically rather than from JSON — used by callers
//! who want to inject ad-hoc patterns on a single
//! [`PatternEngine::scan_entities`] call via
//! [`ScanContext::extra_patterns`] without rebuilding the engine.
//!
//! [`ScanContext`]: crate::ScanContext
//! [`ScanContext::extra_patterns`]: crate::ScanContext::extra_patterns
//! [`PatternEngine::scan_entities`]: crate::PatternEngine::scan_entities

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use serde::{Deserialize, Serialize};

use super::context_rule::ContextRule;
use super::pattern::{MatchSource, Pattern};
use super::pattern_metadata::PatternMetadata;

/// A pattern constructed at runtime, suitable for per-call injection
/// through [`ScanContext::extra_patterns`].
///
/// Same shape as the built-in `JsonPattern` but reachable from code
/// without going through JSON deserialization. Implements the
/// crate-private `Pattern` trait so the engine's compile + scan path
/// treats it identically to a registry-defined pattern.
///
/// Construct via [`RuntimePattern::new`], then layer optional
/// [`with_context`] / [`with_metadata`].
///
/// [`ScanContext::extra_patterns`]: crate::engine::ScanContext::extra_patterns
/// [`with_context`]: Self::with_context
/// [`with_metadata`]: Self::with_metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePattern {
    /// Unique name identifying this pattern.
    pub name: String,
    /// High-level entity category. `None` falls back to
    /// [`EntityCategory::Unresolved`] when the engine emits an entity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<EntityCategory>,
    /// Specific entity kind. `None` falls back to
    /// [`EntityKind::Unresolved`] when the engine emits an entity.
    #[serde(
        default,
        rename = "entity_type",
        skip_serializing_if = "Option::is_none"
    )]
    pub entity_kind: Option<EntityKind>,
    /// Match source: regex, glob, or dictionary lookup.
    #[serde(flatten)]
    pub match_source: MatchSource,
    /// Optional co-occurrence rule for confidence boosting.
    #[serde(default)]
    pub context: Option<ContextRule>,
    /// Optional metadata.
    #[serde(default)]
    pub metadata: PatternMetadata,
}

impl RuntimePattern {
    /// Construct a new runtime pattern with no category or kind tag.
    /// Both default to `Unresolved` at scan time. Use
    /// [`with_category`] / [`with_kind`] to set them.
    ///
    /// [`with_category`]: Self::with_category
    /// [`with_kind`]: Self::with_kind
    pub fn new(name: impl Into<String>, match_source: MatchSource) -> Self {
        Self {
            name: name.into(),
            category: None,
            entity_kind: None,
            match_source,
            context: None,
            metadata: PatternMetadata::default(),
        }
    }

    /// Tag this pattern with an entity category.
    pub fn with_category(mut self, category: EntityCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Tag this pattern with a specific entity kind.
    pub fn with_kind(mut self, entity_kind: EntityKind) -> Self {
        self.entity_kind = Some(entity_kind);
        self
    }

    /// Attach a co-occurrence rule.
    pub fn with_context(mut self, ctx: ContextRule) -> Self {
        self.context = Some(ctx);
        self
    }

    /// Attach metadata.
    pub fn with_metadata(mut self, metadata: PatternMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl Pattern for RuntimePattern {
    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> EntityCategory {
        self.category.unwrap_or(EntityCategory::Unresolved)
    }

    fn entity_kind(&self) -> EntityKind {
        self.entity_kind.unwrap_or(EntityKind::Unresolved)
    }

    fn match_source(&self) -> &MatchSource {
        &self.match_source
    }

    fn context(&self) -> Option<&ContextRule> {
        self.context.as_ref()
    }

    fn metadata(&self) -> &PatternMetadata {
        &self.metadata
    }
}
