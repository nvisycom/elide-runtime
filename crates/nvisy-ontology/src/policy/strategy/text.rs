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
    /// Replace with a consistent pseudonym.
    Pseudonymize,
    /// Replace with a vault-backed reversible token.
    Tokenize {
        /// Identifier of the token vault.
        #[serde(default)]
        vault_id: Option<String>,
    },
}

impl Default for TextStrategy {
    fn default() -> Self {
        Self::Replace {
            placeholder: String::new(),
        }
    }
}

/// Parameter-less tag for each [`TextStrategy`] variant. Used in
/// `Policy<Text>::method_dominance` to declare tiebreaker order
/// among Partial-profile methods on overlapping spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextMethodTag {
    /// Tag for [`TextStrategy::Mask`].
    Mask,
    /// Tag for [`TextStrategy::Replace`].
    Replace,
    /// Tag for [`TextStrategy::Hash`].
    Hash,
    /// Tag for [`TextStrategy::Encrypt`].
    Encrypt,
    /// Tag for [`TextStrategy::Remove`].
    Remove,
    /// Tag for [`TextStrategy::Pseudonymize`].
    Pseudonymize,
    /// Tag for [`TextStrategy::Tokenize`].
    Tokenize,
}

impl RedactionStrategy for TextStrategy {
    type Tag = TextMethodTag;

    /// - [`Hash`](Self::Hash), [`Encrypt`](Self::Encrypt),
    ///   [`Pseudonymize`](Self::Pseudonymize), [`Tokenize`](Self::Tokenize)
    ///   are [`Recoverable`](LeakProfile::Recoverable) — original
    ///   value recoverable with the right metadata (entity list +
    ///   algorithm for Hash, key for Encrypt, mapping for
    ///   Pseudonymize, vault for Tokenize).
    /// - [`Mask`](Self::Mask), [`Replace`](Self::Replace) are
    ///   [`Partial`](LeakProfile::Partial) — original is gone but
    ///   position and length leak through the output.
    /// - [`Remove`](Self::Remove) is
    ///   [`Irrecoverable`](LeakProfile::Irrecoverable) — span
    ///   deleted, no trace.
    fn leak_profile(&self) -> LeakProfile {
        match self {
            Self::Hash | Self::Encrypt { .. } | Self::Pseudonymize | Self::Tokenize { .. } => {
                LeakProfile::Recoverable
            }
            Self::Mask { .. } | Self::Replace { .. } => LeakProfile::Partial,
            Self::Remove => LeakProfile::Irrecoverable,
        }
    }

    fn method_tag(&self) -> Self::Tag {
        match self {
            Self::Mask { .. } => TextMethodTag::Mask,
            Self::Replace { .. } => TextMethodTag::Replace,
            Self::Hash => TextMethodTag::Hash,
            Self::Encrypt { .. } => TextMethodTag::Encrypt,
            Self::Remove => TextMethodTag::Remove,
            Self::Pseudonymize => TextMethodTag::Pseudonymize,
            Self::Tokenize { .. } => TextMethodTag::Tokenize,
        }
    }
}
