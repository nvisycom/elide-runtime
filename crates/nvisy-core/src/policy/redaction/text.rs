//! [`TextRedaction`]: the operator spec a text-modality policy rule
//! carries.
//!
//! Built-in variants ([`Replace`], [`Mask`], [`Hash`],
//! [`Redact`], [`Keep`]) are instantiated per-call from the
//! params on the variant — no registry round-trip.
//! [`TextRedaction::Custom`] names a deployment-registered operator
//! looked up in the [`RedactionRegistry<Text>`] at apply time.
//!
//! ## Why no `Encrypt` variant?
//!
//! [`Encrypt`] needs 32 bytes of raw key material plus an
//! [`Aes256Gcm`] instance. Putting either in a TOML / JSON wire
//! shape is unsafe (key leakage in version control, log dumps) and
//! awkward (binary blobs in declarative config). Deployments that
//! want reversible AES-256-GCM redaction build an [`Encrypt`] in
//! Rust code, register it as a custom anonymizer on the
//! [`RedactionRegistry<Text>`], and reference it from policy with
//! `{ kind = "custom", id = "..." }`.
//!
//! [`Replace`]: nvisy_toolkit::redaction::anonymizer::Replace
//! [`Mask`]: nvisy_toolkit::redaction::anonymizer::Mask
//! [`Hash`]: nvisy_toolkit::redaction::anonymizer::Hash
//! [`Redact`]: nvisy_toolkit::redaction::anonymizer::Redact
//! [`Keep`]: nvisy_toolkit::redaction::anonymizer::Keep
//! [`Encrypt`]: nvisy_toolkit::redaction::anonymizer::Encrypt
//! [`Aes256Gcm`]: https://docs.rs/aes-gcm
//! [`RedactionRegistry<Text>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::modality::Text;
use nvisy_toolkit::redaction::AnonymizerId;
use nvisy_toolkit::redaction::anonymizer::HashAlgorithm;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` text rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextRedaction {
    /// Substitute the span with a fixed template. Supports
    /// `{entity_kind}` / `{value}` placeholders. Default template
    /// is `[{entity_kind}]`.
    Replace {
        /// Template string. Default `[{entity_kind}]`.
        #[serde(default = "default_replace_template")]
        template: String,
    },
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
    /// One-way SHA-2 hash with optional salt.
    Hash {
        /// SHA-256 (default) or SHA-512.
        #[serde(default)]
        algorithm: HashAlgorithm,
        /// Salt prepended to the value before hashing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        salt: Option<String>,
    },
    /// Delete the matched span entirely.
    Redact,
    /// Pass the value through unchanged.
    Keep,
    /// Look up a deployment-registered custom operator by id.
    Custom {
        /// Id under which the operator was registered in the
        /// [`RedactionRegistry<Text>`].
        ///
        /// [`RedactionRegistry<Text>`]: nvisy_toolkit::redaction::RedactionRegistry
        id: AnonymizerId<Text>,
    },
}

fn default_replace_template() -> String {
    "[{entity_kind}]".to_string()
}

fn default_mask_char() -> char {
    '*'
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}
