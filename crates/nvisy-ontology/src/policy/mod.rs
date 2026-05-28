//! Policy types, rules, and governance structures.
//!
//! A [`Policy<M>`] is a named, versioned governance artifact
//! containing strategy rules for entity redaction and retention rules
//! for data lifecycle management. Policies are typed per modality:
//! one envelope, one modality, one policy stack.
//!
//! Collections of policies are plain `Vec<Policy<M>>` — held in
//! precedence order, index `0` is the highest-precedence policy.

mod condition;
mod retention;
mod selector;
mod strategy;

use derive_builder::Builder;
use schemars::JsonSchema;
use semver::Version;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::condition::Condition;
pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::selector::EntitySelector;
pub use self::strategy::{
    Action, AudioStrategy, ImageStrategy, StrategyPolicy, TabularStrategy, TextStrategy,
};
use crate::modality::Modality;

/// A named, versioned governance policy for one modality.
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct Policy<M: Modality> {
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
    /// Fallback strategy for entities that no
    /// [`StrategyPolicy`] selector matches.
    ///
    /// Precedence: the redaction evaluator first walks
    /// [`Self::strategies`] in declaration order and applies the
    /// first matching `StrategyPolicy`. Only when no selector
    /// matches does it fall back to `default_strategy`. When this
    /// field is `None` and no selector matches, the entity is left
    /// un-redacted (and surfaces as a leak in post-redaction
    /// validation). The selection logic itself lives in the engine
    /// (`nvisy-engine`) — this doc is the contract the ontology
    /// type imposes on consumers.
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_strategy: Option<M::Strategy>,
    /// Entity redaction strategies.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strategies: Vec<StrategyPolicy<M>>,
    /// Data retention lifecycle rules.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}

impl<M: Modality> Policy<M> {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder<M> {
        PolicyBuilder::default()
    }
}

/// A reference to a stored [`Policy`] tagged with the precedence it
/// should take when applied alongside other policies.
///
/// Lower [`precedence`] wins: a ref with `precedence: 0` is the most
/// authoritative ("override"), higher numbers are layered underneath
/// (org defaults, etc.). Ties are resolved by insertion order (stable).
///
/// [`precedence`]: PolicyRef::precedence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRef {
    /// Identifier of the previously uploaded policy.
    pub id: Uuid,
    /// Application precedence (lower = higher precedence).
    pub precedence: u32,
}
