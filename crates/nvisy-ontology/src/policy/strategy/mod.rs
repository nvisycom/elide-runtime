//! Redaction strategies, modality-tagged via a single [`Strategy`]
//! enum.
//!
//! Each modality has its own concrete strategy enum (carrying the
//! methods that make sense for that data shape). [`Strategy`]
//! wraps them so a single [`Policy`] can carry every modality's
//! strategies without becoming generic.
//!
//! [`Policy`]: super::Policy

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
use crate::modality::RedactionStrategy;

/// Modality-tagged redaction strategy.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum Strategy {
    /// Text-modality strategy.
    Text(TextStrategy),
    /// Image-modality strategy.
    Image(ImageStrategy),
    /// Audio-modality strategy.
    Audio(AudioStrategy),
    /// Tabular-modality strategy.
    Tabular(TabularStrategy),
}

impl RedactionStrategy for Strategy {
    fn is_reversible(&self) -> bool {
        match self {
            Self::Text(s) => s.is_reversible(),
            Self::Image(s) => s.is_reversible(),
            Self::Audio(s) => s.is_reversible(),
            Self::Tabular(s) => s.is_reversible(),
        }
    }
}

/// The action a policy performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Apply a redaction to the matched entity.
    Redact {
        /// Redaction strategy to apply.
        strategy: Strategy,
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
#[serde(rename_all = "camelCase")]
pub struct StrategyPolicy {
    /// Which entities this policy applies to.
    pub selector: EntitySelector,
    /// What this policy does when it matches.
    pub action: Action,
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

impl StrategyPolicy {
    /// Evaluation priority (lower = higher precedence, default 0).
    pub fn priority(&self) -> i32 {
        self.priority.unwrap_or(0)
    }
}
