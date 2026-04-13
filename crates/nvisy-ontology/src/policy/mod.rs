//! Policy types, rules, and governance structures.
//!
//! A [`Policy`] is a named, versioned governance artifact containing
//! strategy rules for entity redaction and retention rules for data
//! lifecycle management.

mod condition;
mod retention;
mod selector;
mod strategy;

use derive_builder::Builder;
use derive_more::{Deref, DerefMut};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::condition::Condition;
pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::selector::EntitySelector;
pub use self::strategy::{
    Action, AudioStrategy, DefaultStrategy, ImageStrategy, Strategy, StrategyPolicy, TextStrategy,
};

/// A named, versioned governance policy.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// Unique identifier for this policy.
    #[builder(default = "Uuid::now_v7()")]
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
    /// Description of the policy's purpose.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Per-modality fallback strategies for unmatched entities.
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "DefaultStrategy::is_empty")]
    pub default_strategy: DefaultStrategy,
    /// Entity redaction strategies.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strategies: Vec<StrategyPolicy>,
    /// Data retention lifecycle rules.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}

impl Policy {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder {
        PolicyBuilder::default()
    }
}

/// A collection of policies to apply during a pipeline run.
#[derive(Debug, Clone, Default, Deref, DerefMut)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Policies {
    /// The policies to evaluate, in order.
    #[deref]
    #[deref_mut]
    pub policies: Vec<Policy>,
}

impl Policies {
    /// All strategy policies across all policies, sorted by priority.
    ///
    /// Returns tuples of `(policy_id, strategy)` so callers can trace
    /// which policy a matched strategy belongs to.
    pub fn all_strategies(&self) -> Vec<(Uuid, &StrategyPolicy)> {
        let mut result: Vec<_> = self
            .policies
            .iter()
            .flat_map(|p| p.strategies.iter().map(move |s| (p.id, s)))
            .collect();
        result.sort_by_key(|(_, s)| s.priority());
        result
    }

    /// All retention policies across all policies.
    pub fn all_retention(&self) -> Vec<&RetentionPolicy> {
        self.policies
            .iter()
            .flat_map(|p| p.retention.iter())
            .collect()
    }

    /// Merged default strategy across all policies.
    ///
    /// Earlier policies take precedence per-modality: if policy A sets
    /// a text default and policy B sets text + image defaults, the result
    /// uses A's text and B's image.
    pub fn default_strategy(&self) -> DefaultStrategy {
        let mut merged = DefaultStrategy::default();
        for policy in &self.policies {
            merged.merge(&policy.default_strategy);
        }
        merged
    }

    /// Look up the effective retention for a given scope.
    pub fn retention_for(&self, scope: RetentionScope) -> Option<Retention> {
        self.all_retention()
            .into_iter()
            .find(|rp| rp.scope == scope)
            .map(|rp| rp.retention)
    }
}
