//! Text and tabular redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modality::{LeakProfile, RedactionStrategy};

const DEFAULT_MASK_CHAR: char = '*';

fn default_mask_char() -> char {
    DEFAULT_MASK_CHAR
}

/// Text redaction strategy with method-specific configuration.
///
/// The [`Default`] impl returns [`Replace`] with an empty placeholder;
/// the engine fills in `[ENTITY_KIND]` when the placeholder is empty,
/// producing a neutral marker so users see something was redacted.
///
/// [`Replace`]: TextStrategy::Replace
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextStrategy {
    /// Replace characters with a mask character.
    Mask {
        /// Character used for masking (default `'*'`).
        #[serde(default = "default_mask_char")]
        mask_char: char,
    },
    /// Substitute with a fixed placeholder string.
    Replace {
        /// Template for the replacement (supports `{entityType}`, `{category}`).
        #[serde(default)]
        placeholder: String,
    },
    /// Replace with a one-way hash.
    Hash,
    /// Encrypt the value; recoverable with the referenced key.
    Encrypt {
        /// Identifier of the encryption key to use.
        key_id: String,
    },
    /// Remove the value entirely.
    Remove,
}

impl Default for TextStrategy {
    fn default() -> Self {
        Self::Replace {
            placeholder: String::new(),
        }
    }
}

impl RedactionStrategy for TextStrategy {
    /// - [`Hash`], [`Encrypt`] are [`Recoverable`] — original value
    ///   recoverable with the right metadata (entity list + algorithm
    ///   for Hash, key for Encrypt).
    /// - [`Mask`], [`Replace`] are [`Partial`] — original is gone but
    ///   position and length leak through the output.
    /// - [`Remove`] is [`Irrecoverable`] — span deleted, no trace.
    ///
    /// [`Hash`]: Self::Hash
    /// [`Encrypt`]: Self::Encrypt
    /// [`Mask`]: Self::Mask
    /// [`Replace`]: Self::Replace
    /// [`Remove`]: Self::Remove
    /// [`Recoverable`]: LeakProfile::Recoverable
    /// [`Partial`]: LeakProfile::Partial
    /// [`Irrecoverable`]: LeakProfile::Irrecoverable
    fn leak_profile(&self) -> LeakProfile {
        match self {
            Self::Hash | Self::Encrypt { .. } => LeakProfile::Recoverable,
            Self::Mask { .. } | Self::Replace { .. } => LeakProfile::Partial,
            Self::Remove => LeakProfile::Irrecoverable,
        }
    }
}
