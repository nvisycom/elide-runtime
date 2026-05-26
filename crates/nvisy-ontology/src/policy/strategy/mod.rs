//! Per-modality redaction strategies.
//!
//! Each modality exposes one strategy enum
//! ([`TextStrategy`], [`ImageStrategy`], [`AudioStrategy`],
//! [`TabularStrategy`]) pairing a redaction method with its
//! parameters. The modality's [`Modality::Strategy`] associated type
//! points at the appropriate one; consumers parameterise their own
//! types over `M::Strategy` (e.g. [`Policy<M>`], [`AuditEntry<M>`]).
//!
//! [`Modality::Strategy`]: crate::modality::Modality::Strategy
//! [`Policy<M>`]: super::Policy
//! [`AuditEntry<M>`]: crate::provenance::AuditEntry

mod audio;
mod image;
mod tabular;
mod text;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::AudioStrategy;
pub use self::image::ImageStrategy;
pub use self::tabular::TabularStrategy;
pub use self::text::TextStrategy;
use super::condition::Condition;
use super::selector::EntitySelector;
use crate::modality::Modality;

/// The action a policy performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: serde::de::DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub enum Action<M: Modality> {
    /// Apply a redaction to the matched entity.
    Redact {
        /// Redaction strategy to apply.
        strategy: M::Strategy,
    },
    /// Require human review before any action is taken.
    Review,
    /// Flag the entity without redacting (for reporting/alerting).
    Alert,
    /// Block processing of the entire document.
    Block,
    /// Suppress a detection (treat as false positive).
    Suppress,
}

/// A single policy: selector + action + conditions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: serde::de::DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct StrategyPolicy<M: Modality> {
    /// Which entities this policy applies to.
    pub selector: EntitySelector,
    /// What this policy does when it matches.
    pub action: Action<M>,
    /// Evaluation priority (lower numbers are evaluated first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Conditions that must all be met for this policy to apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Whether this policy is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl<M: Modality> StrategyPolicy<M> {
    /// Evaluation priority (lower = higher precedence, default 0).
    pub fn priority(&self) -> i32 {
        self.priority.unwrap_or(0)
    }
}
