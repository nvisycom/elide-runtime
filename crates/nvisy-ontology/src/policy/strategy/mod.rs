//! Redaction strategies for text, image, and audio modalities.
//!
//! Each per-modality strategy ([`TextStrategy`], [`ImageStrategy`],
//! [`AudioStrategy`]) pairs a redaction method with its configuration
//! parameters. [`Strategy`] composes them: it carries an optional
//! per-modality strategy for each of text/image/audio, falling back to
//! that modality's [`Default`] when unset.
//!
//! A single [`Strategy`] value can be used both as a per-rule directive
//! ("when this rule matches, use these methods") and as a per-policy
//! default ("for any modality not specified by a matching rule, use
//! these methods"). The applicator looks up the appropriate modality
//! at runtime and never needs to dispatch on a strategy-vs-location
//! modality mismatch — every modality always resolves to something.

mod audio;
mod image;
mod text;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::AudioStrategy;
pub use self::image::ImageStrategy;
pub use self::text::TextStrategy;
use super::condition::Condition;
use super::selector::EntitySelector;
use crate::modality::AnyModality;

/// A composed redaction strategy across all modalities.
///
/// Each field is optional: an unset field means "use the modality's
/// own [`Default`]" when the strategy is applied. The per-modality
/// accessors ([`text_or_default`], [`image_or_default`],
/// [`audio_or_default`]) resolve to a concrete strategy without ever
/// failing.
///
/// [`text_or_default`]: Strategy::text_or_default
/// [`image_or_default`]: Strategy::image_or_default
/// [`audio_or_default`]: Strategy::audio_or_default
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Strategy {
    /// Override for text entities (and the text view of tabular entities).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextStrategy>,
    /// Override for image entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageStrategy>,
    /// Override for audio entities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioStrategy>,
}

impl Strategy {
    /// Construct from a single text strategy.
    pub fn text(strategy: TextStrategy) -> Self {
        Self {
            text: Some(strategy),
            ..Self::default()
        }
    }

    /// Construct from a single image strategy.
    pub fn image(strategy: ImageStrategy) -> Self {
        Self {
            image: Some(strategy),
            ..Self::default()
        }
    }

    /// Construct from a single audio strategy.
    pub fn audio(strategy: AudioStrategy) -> Self {
        Self {
            audio: Some(strategy),
            ..Self::default()
        }
    }

    /// `true` if no per-modality strategy is set.
    ///
    /// An empty strategy is valid at apply time (every modality falls
    /// back to its default) but a rule with [`Action::Redact`] and an
    /// empty strategy is a configuration smell — the validator emits
    /// a warning.
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.image.is_none() && self.audio.is_none()
    }

    /// Merge `other` into `self`, filling in modality slots that
    /// `self` does not already set.
    ///
    /// Self's values take precedence per-modality. Used to compose a
    /// rule-level strategy with the policy-level default strategy.
    pub fn merge(&mut self, other: &Strategy) {
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

    /// Concrete [`TextStrategy`] to apply, falling back to
    /// [`TextStrategy::default`] when no override is set.
    pub fn text_or_default(&self) -> TextStrategy {
        self.text.clone().unwrap_or_default()
    }

    /// Concrete [`ImageStrategy`] to apply, falling back to
    /// [`ImageStrategy::default`] when no override is set.
    pub fn image_or_default(&self) -> ImageStrategy {
        self.image.clone().unwrap_or_default()
    }

    /// Concrete [`AudioStrategy`] to apply, falling back to
    /// [`AudioStrategy::default`] when no override is set.
    pub fn audio_or_default(&self) -> AudioStrategy {
        self.audio.clone().unwrap_or_default()
    }

    /// Whether the redaction at `location` is reversible (the original
    /// value can be recovered from the redacted output).
    ///
    /// Only [`TextStrategy::Encrypt`] and [`TextStrategy::Tokenize`]
    /// are reversible: Encrypt uses key-based decryption, Tokenize
    /// uses vault-based detokenization. All other strategies are
    /// destructive.
    pub fn is_reversible_for(&self, location: &AnyModality) -> bool {
        match location {
            AnyModality::Text(_) | AnyModality::Tabular(_) => matches!(
                self.text_or_default(),
                TextStrategy::Encrypt { .. } | TextStrategy::Tokenize { .. },
            ),
            AnyModality::Image(_) | AnyModality::Audio(_) => false,
        }
    }
}

/// The action a strategy policy performs when it matches an entity.
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
