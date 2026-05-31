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
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use self::audio::{AudioMethodTag, AudioStrategy};
pub use self::image::{ImageMethodTag, ImageStrategy};
pub use self::tabular::TabularStrategy;
pub use self::text::TextStrategy;
use super::condition::Condition;
use super::selector::EntitySelector;
use crate::modality::Modality;

/// The action a policy rule performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub enum Action<M: Modality> {
    /// Apply a redaction to the matched entity.
    Redact {
        /// Redaction strategy to apply.
        strategy: M::Strategy,
    },
    /// Suppress a detection (treat as false positive). The entity is
    /// not redacted; an audit entry records the suppression.
    Suppress,
}

/// One rule inside a [`Policy`]: a selector, an action, optional
/// conditions, and an enabled flag.
///
/// Rules are ordered inside their owning policy; the first matching
/// rule wins. There is no separate `priority` field — re-ordering
/// rules in the policy file is how authors change priority.
///
/// [`Policy`]: super::Policy
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(
    rename_all = "camelCase",
    bound(
        serialize = "M::Strategy: Serialize",
        deserialize = "M::Strategy: DeserializeOwned",
    )
)]
#[schemars(bound = "M::Strategy: JsonSchema")]
pub struct PolicyRule<M: Modality> {
    /// Which entities this rule applies to.
    pub selector: EntitySelector,
    /// What this rule does when it matches.
    pub action: Action<M>,
    /// Conditions that must all be met for this rule to apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Whether this rule is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}
