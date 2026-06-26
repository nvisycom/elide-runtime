//! [`TextRedaction`]: the operator spec a text-modality policy rule
//! carries.
//!
//! Each variant mirrors an elide built-in operator the engine
//! constructs at apply time:
//!
//! - [`TextRedaction::Erase`] → [`elide::redaction::operators::Erase`]
//! - [`TextRedaction::Keep`] → [`elide::redaction::operators::Keep`]
//! - [`TextRedaction::Mask`] → [`elide::redaction::operators::Mask`]
//! - [`TextRedaction::Replace`] → [`elide::redaction::operators::Replace`]
//! - [`TextRedaction::Hash`] → [`elide::redaction::operators::Sha2Hash`]
//! - [`TextRedaction::Pseudonymize`] →
//!   [`elide::redaction::operators::Pseudonymize`]
//! - [`TextRedaction::Encrypt`] →
//!   [`elide::redaction::operators::AesEncrypt`] (engine wires the
//!   per-tenant key provider)
//!
//! No `Custom` escape hatch — every operator the wire format admits
//! is predefined. New built-ins land in elide first, then surface
//! here as new variants.

use elide::redaction::operators::Sha2Algorithm;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// SHA-2 variant for the [`TextRedaction::Hash`] operator.
///
/// Spec mirror of elide's [`Sha2Algorithm`]; the [`From`] impl below
/// keeps the wire vocabulary independent of upstream's serde gating.
///
/// [`Sha2Algorithm`]: elide::redaction::operators::Sha2Algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    /// SHA-256 — 32-byte digest, 64-char hex.
    #[default]
    Sha256,
    /// SHA-512 — 64-byte digest, 128-char hex.
    Sha512,
}

impl From<HashAlgorithm> for Sha2Algorithm {
    fn from(value: HashAlgorithm) -> Self {
        match value {
            HashAlgorithm::Sha256 => Sha2Algorithm::Sha256,
            HashAlgorithm::Sha512 => Sha2Algorithm::Sha512,
        }
    }
}

/// Operator spec a `redact` text rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextRedaction {
    /// Delete the matched span entirely.
    Erase,
    /// Pass the value through unchanged.
    Keep,
    /// Character-replacement masking. Leaves `keep_prefix` leading
    /// and `keep_suffix` trailing characters visible; masks the
    /// rest with `mask_char`.
    Mask {
        /// The character that replaces masked positions.
        #[serde(default = "default_mask_char")]
        mask_char: char,
        /// Characters to leave unmasked at the start of the value.
        /// `0` (the default) masks from the start.
        #[serde(default, skip_serializing_if = "is_zero")]
        keep_prefix: usize,
        /// Characters to leave unmasked at the end of the value.
        /// `0` (the default) masks through to the end.
        #[serde(default, skip_serializing_if = "is_zero")]
        keep_suffix: usize,
    },
    /// Substitute the span with a fixed template. Supports
    /// `{label}` / `{value}` / `{coref}` placeholders.
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
    /// reads the same surrogate. The engine wires a per-request
    /// vault + the default [`RandomToken`] generator.
    ///
    /// [`RandomToken`]: elide::redaction::generator::RandomToken
    Pseudonymize,
    /// Reversible AES-256-GCM ciphertext. The engine wires the
    /// per-tenant [`KeyProvider`] so raw key material never lives in
    /// serialised policy.
    ///
    /// [`KeyProvider`]: elide::redaction::key_provider::KeyProvider
    Encrypt,
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
