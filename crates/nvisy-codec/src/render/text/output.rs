//! Text redaction output type.

use serde::{Deserialize, Serialize};

/// Text redaction output — records the method used and its replacement data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextRedactionOutput {
    /// Characters replaced with a mask character.
    Mask {
        replacement: String,
        mask_char: char,
    },
    /// Substituted with a fixed placeholder string.
    Replace { replacement: String },
    /// Replaced with a one-way hash.
    Hash { hash_value: String },
    /// Encrypted; recoverable with the referenced key.
    Encrypt { ciphertext: String, key_id: String },
    /// Removed entirely from the output.
    Remove,
    /// Replaced with a synthetically generated value.
    Synthesize { replacement: String },
    /// Replaced with a consistent pseudonym.
    Pseudonymize { pseudonym: String },
    /// Replaced with a vault-backed reversible token.
    Tokenize {
        token: String,
        vault_id: Option<String>,
    },
    /// Aggregated into a range or bucket.
    Aggregate { replacement: String },
    /// Generalized to a less precise value.
    Generalize {
        replacement: String,
        level: Option<u32>,
    },
    /// Date shifted by a consistent offset.
    DateShift {
        replacement: String,
        offset_days: i64,
    },
}

impl TextRedactionOutput {
    /// Returns the text replacement string, regardless of specific method.
    ///
    /// Returns `None` for [`Remove`](Self::Remove) — the caller should
    /// treat that as an empty string (span deleted).
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Mask { replacement, .. } => Some(replacement),
            Self::Replace { replacement } => Some(replacement),
            Self::Hash { hash_value } => Some(hash_value),
            Self::Encrypt { ciphertext, .. } => Some(ciphertext),
            Self::Remove => None,
            Self::Synthesize { replacement } => Some(replacement),
            Self::Pseudonymize { pseudonym } => Some(pseudonym),
            Self::Tokenize { token, .. } => Some(token),
            Self::Aggregate { replacement } => Some(replacement),
            Self::Generalize { replacement, .. } => Some(replacement),
            Self::DateShift { replacement, .. } => Some(replacement),
        }
    }
}
