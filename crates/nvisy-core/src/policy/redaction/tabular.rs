//! [`TabularRedaction`]: the operator spec a tabular-modality policy
//! rule carries.
//!
//! Tabular is `TextBacked` in elide — cells reuse the text payload
//! and replacement types — so the operator catalogue is identical
//! to [`TextRedaction`]. The spec is structurally the same; we
//! declare it as a separate enum so the wire format can diverge if
//! tabular ever grows table-shape-aware operators (per-column
//! masking, header-preserving transforms, …) that wouldn't make
//! sense on text.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::text::HashAlgorithm;

/// Operator spec a `redact` tabular rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularRedaction {
    /// Delete the matched cell entirely.
    Erase,
    /// Pass the value through unchanged.
    Keep,
    /// Character-replacement masking.
    Mask {
        /// The character that replaces masked positions.
        #[serde(default = "default_mask_char")]
        mask_char: char,
        /// Characters to leave unmasked at the start of the value.
        #[serde(default, skip_serializing_if = "is_zero")]
        keep_prefix: usize,
        /// Characters to leave unmasked at the end of the value.
        #[serde(default, skip_serializing_if = "is_zero")]
        keep_suffix: usize,
    },
    /// Substitute the cell with a fixed template (`{label}`,
    /// `{value}`, `{coref}` placeholders).
    Replace {
        /// Template string. Default `[{label}]`.
        #[serde(default = "default_replace_template")]
        template: String,
    },
    /// One-way SHA-2 hash with optional salt.
    Hash {
        /// SHA-256 (default) or SHA-512.
        #[serde(default)]
        algorithm: HashAlgorithm,
        /// Salt prepended to the value before hashing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        salt: Option<String>,
    },
    /// Vault-backed pseudonym: every mention of the same entity
    /// reads the same surrogate.
    Pseudonymize,
    /// Reversible AES-256-GCM ciphertext. The engine wires the
    /// per-tenant key provider.
    Encrypt,
    /// Drop the entire row a matched cell sits in.
    DropRow,
    /// Drop the entire column a matched cell sits in (header
    /// included).
    DropColumn,
}

fn default_replace_template() -> String {
    "[{label}]".to_string()
}

fn default_mask_char() -> char {
    '*'
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}
