//! Redaction strategies for text, image, and audio modalities.
//!
//! Each per-modality strategy ([`TextStrategy`], [`ImageStrategy`],
//! [`AudioStrategy`]) pairs a redaction method with its configuration
//! parameters. [`Strategy`] unifies them under a single tagged enum for
//! policy rules and pipeline decisions.

mod audio;
mod image;
mod text;

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::AudioStrategy;
pub use self::image::ImageStrategy;
pub use self::text::TextStrategy;
use super::condition::Condition;
use super::selector::EntitySelector;
use crate::entity::Location;

/// Unified redaction strategy across all modalities.
///
/// Wraps a per-modality strategy variant carrying the method and its
/// configuration parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(From, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Strategy {
    /// Text/tabular redaction strategy.
    Text(TextStrategy),
    /// Image redaction strategy.
    Image(ImageStrategy),
    /// Audio redaction strategy.
    Audio(AudioStrategy),
}

impl Strategy {
    /// Whether the redaction is reversible (the original value can be
    /// recovered from the redacted output).
    ///
    /// Only [`TextStrategy::Encrypt`] and [`TextStrategy::Tokenize`]
    /// are reversible: Encrypt uses key-based decryption, Tokenize
    /// uses vault-based detokenization. All other strategies are
    /// destructive.
    pub fn is_reversible(&self) -> bool {
        matches!(
            self,
            Self::Text(TextStrategy::Encrypt { .. } | TextStrategy::Tokenize { .. })
        )
    }
}

/// Per-modality fallback strategies for entities that match no explicit rule.
///
/// Each field is optional: `None` means no default for that modality
/// (unmatched entities in that modality are left unredacted). When
/// multiple policies are combined, earlier policies take precedence
/// per-modality via [`DefaultStrategy::merge`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DefaultStrategy {
    /// Fallback strategy for text entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextStrategy>,
    /// Fallback strategy for image entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageStrategy>,
    /// Fallback strategy for audio entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioStrategy>,
}

impl DefaultStrategy {
    /// Returns `true` if no default strategy is set for any modality.
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.image.is_none() && self.audio.is_none()
    }

    /// Merge `other` into `self`, filling in gaps without overriding
    /// already-set modalities. Earlier policies take precedence.
    pub fn merge(&mut self, other: &DefaultStrategy) {
        if self.text.is_none() {
            self.text = other.text.clone();
        }
        if self.image.is_none() {
            self.image = other.image.clone();
        }
        if self.audio.is_none() {
            self.audio = other.audio.clone();
        }
    }

    /// Look up the default strategy for a given entity location.
    pub fn for_location(&self, location: &Location) -> Option<Strategy> {
        match location {
            Location::Text(_) | Location::Tabular(_) => self.text.clone().map(Strategy::Text),
            Location::Image(_) => self.image.clone().map(Strategy::Image),
            Location::Audio(_) => self.audio.clone().map(Strategy::Audio),
        }
    }
}

/// The action a strategy policy performs when it matches an entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
#[non_exhaustive]
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

/// A single entity redaction strategy: selector + action + conditions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StrategyPolicy {
    /// Which entities this strategy applies to.
    pub selector: EntitySelector,
    /// What this strategy does when it matches.
    pub action: Action,
    /// Evaluation priority (lower numbers are evaluated first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Conditions that must all be met for this strategy to apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Whether this strategy is active.
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
