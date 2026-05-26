//! Runtime-constructed [`RuntimePattern`] for per-call [`PatternContext`] extras.
//!
//! Built programmatically rather than from JSON — used by callers
//! who want to inject ad-hoc patterns on a single
//! [`PatternEngine::scan`] call via
//! [`PatternContext::extra_patterns`] without rebuilding the engine.
//!
//! [`PatternContext`]: crate::PatternContext
//! [`PatternContext::extra_patterns`]: crate::PatternContext::extra_patterns
//! [`PatternEngine::scan`]: crate::PatternEngine::scan

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use serde::{Deserialize, Serialize};

use super::context_rule::ContextRule;
use super::pattern::{MatchSource, Pattern};
use super::pattern_metadata::PatternMetadata;

/// A pattern constructed at runtime, suitable for per-call injection
/// through [`PatternContext::extra_patterns`].
///
/// Same shape as the built-in `JsonPattern` but reachable from code
/// without going through JSON deserialization. Implements the
/// crate-private `Pattern` trait so the engine's compile + scan path
/// treats it identically to a registry-defined pattern.
///
/// Construct via [`RuntimePattern::new`], then layer an optional
/// co-occurrence rule with [`with_context`].
///
/// [`PatternContext::extra_patterns`]: crate::engine::PatternContext::extra_patterns
/// [`with_context`]: Self::with_context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePattern {
    /// Unique name identifying this pattern.
    pub name: String,
    /// High-level entity category. Defaults to
    /// [`EntityCategory::Unresolved`] when the caller doesn't tag
    /// the pattern with anything more specific.
    #[serde(default = "default_category")]
    pub category: EntityCategory,
    /// Specific entity kind. Defaults to [`EntityKind::Unresolved`]
    /// when untagged.
    #[serde(default = "default_kind", rename = "entity_type")]
    pub entity_kind: EntityKind,
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

fn default_category() -> EntityCategory {
    EntityCategory::Unresolved
}

fn default_kind() -> EntityKind {
    EntityKind::Unresolved
}

impl RuntimePattern {
    /// Construct a new runtime pattern with both `category` and
    /// `entity_kind` set to `Unresolved`. Use [`with_category`] /
    /// [`with_kind`] to tag, and [`with_context`] for an optional
    /// co-occurrence rule.
    ///
    /// [`with_category`]: Self::with_category
    /// [`with_kind`]: Self::with_kind
    /// [`with_context`]: Self::with_context
    pub fn new(name: impl Into<String>, match_source: MatchSource) -> Self {
        Self {
            name: name.into(),
            category: EntityCategory::Unresolved,
            entity_kind: EntityKind::Unresolved,
            match_source,
            context: None,
            metadata: PatternMetadata::default(),
        }
    }

    /// Tag this pattern with an entity category.
    pub fn with_category(mut self, category: EntityCategory) -> Self {
        self.category = category;
        self
    }

    /// Tag this pattern with a specific entity kind.
    pub fn with_kind(mut self, entity_kind: EntityKind) -> Self {
        self.entity_kind = entity_kind;
        self
    }

    /// Attach a co-occurrence rule.
    pub fn with_context(mut self, ctx: ContextRule) -> Self {
        self.context = Some(ctx);
        self
    }
}

impl Pattern for RuntimePattern {
    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> EntityCategory {
        self.category
    }

    fn entity_kind(&self) -> EntityKind {
        self.entity_kind
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
