//! Text redaction specification with method-specific configuration.

use serde::{Deserialize, Serialize};

/// Text redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextRedactionSpec {
    /// Replace characters with a mask character.
    Mask {
        /// Character used for masking (default `'*'`).
        #[serde(default = "default_mask_char")]
        mask_char: char,
    },
    /// Substitute with a fixed placeholder string.
    Replace {
        /// Template for the replacement (supports `{entityType}`, `{category}`, `{value}`).
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
    /// Replace with a synthetically generated value.
    Synthesize,
    /// Replace with a consistent pseudonym.
    Pseudonymize,
    /// Replace with a vault-backed reversible token.
    Tokenize {
        /// Identifier of the token vault.
        #[serde(default)]
        vault_id: Option<String>,
    },
    /// Aggregate into a range or bucket.
    Aggregate,
    /// Generalize to a less precise value.
    Generalize {
        /// Generalization level (1 = city, 2 = state, etc.).
        #[serde(default)]
        level: Option<u32>,
    },
    /// Shift dates by a consistent offset.
    DateShift {
        /// Fixed offset in days (0 = engine picks a random offset).
        #[serde(default)]
        offset_days: i64,
    },
}

/// Default mask character for text redaction.
pub const DEFAULT_MASK_CHAR: char = '*';

fn default_mask_char() -> char {
    DEFAULT_MASK_CHAR
}
