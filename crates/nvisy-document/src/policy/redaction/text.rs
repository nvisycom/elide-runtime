//! [`TextRedaction`]: the operator spec a text-modality policy rule
//! carries.
//!
//! Built-in variants ([`Replace`][r], [`Mask`][m], [`Hash`][h],
//! [`Redact`][rd], [`Keep`][k]) are instantiated per-call from the
//! params on the variant — no registry round-trip. [`Custom`][c]
//! names a deployment-registered operator looked up in the
//! [`RedactionRegistry<Text>`] at apply time.
//!
//! ## Why no `Encrypt` variant?
//!
//! [`Encrypt`][e] needs 32 bytes of raw key material plus an
//! [`Aes256Gcm`][gcm] instance. Putting either in a TOML / JSON wire
//! shape is unsafe (key leakage in version control, log dumps) and
//! awkward (binary blobs in declarative config). Deployments that
//! want reversible AES-256-GCM redaction build an [`Encrypt`][e] in
//! Rust code, register it as a custom anonymizer on the
//! [`RedactionRegistry<Text>`], and reference it from policy with
//! `{ kind = "custom", id = "..." }`.
//!
//! [r]: nvisy_toolkit::redaction::builtin::Replace
//! [m]: nvisy_toolkit::redaction::builtin::Mask
//! [h]: nvisy_toolkit::redaction::builtin::Hash
//! [rd]: nvisy_toolkit::redaction::builtin::Redact
//! [k]: nvisy_toolkit::redaction::builtin::Keep
//! [e]: nvisy_toolkit::redaction::builtin::Encrypt
//! [gcm]: https://docs.rs/aes-gcm
//! [c]: TextRedaction::Custom
//! [`RedactionRegistry<Text>`]: nvisy_toolkit::redaction::RedactionRegistry

use nvisy_core::modality::Text;
use nvisy_toolkit::redaction::AnonymizerId;
use nvisy_toolkit::redaction::builtin::HashAlgorithm;
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
    /// Character-replacement masking.
    Mask {
        /// The character that replaces masked positions.
        #[serde(default = "default_mask_char")]
        mask_char: char,
        /// How many characters to mask. `None` masks the whole value.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chars_to_mask: Option<usize>,
        /// When `true`, masking starts from the end of the value.
        #[serde(default)]
        from_end: bool,
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
