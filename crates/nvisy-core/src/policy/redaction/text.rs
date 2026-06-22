//! [`TextRedaction`]: the operator spec a text-modality policy rule
//! carries.
//!
//! Spec types — the serialisable, author-facing wire shape. The
//! engine compiles each variant into the matching
//! [`elide::redaction::operators`] instance at apply time.
//!
//! Built-in variants ([`Replace`], [`Mask`], [`Hash`], [`Erase`],
//! [`Keep`]) ship in elide. [`TextRedaction::Custom`] names a
//! deployment-registered operator looked up by [`OperatorId`].
//!
//! ## Why no `Encrypt` variant?
//!
//! Reversible AES-256-GCM needs 32 bytes of raw key material plus an
//! `Aes256Gcm` instance. Putting either in a TOML / JSON wire shape
//! is unsafe (key leakage in version control, log dumps) and awkward
//! (binary blobs in declarative config). Deployments that want
//! reversible AES-256-GCM redaction build an encrypt operator in
//! Rust code, register it via [`OperatorId`], and reference it from
//! policy with `{ kind = "custom", id = "..." }`.
//!
//! [`Replace`]: elide::redaction::operators::Replace
//! [`Mask`]: elide::redaction::operators::Mask
//! [`Hash`]: elide::redaction::operators::Hash
//! [`Erase`]: elide::redaction::operators::Erase
//! [`Keep`]: elide::redaction::operators::Keep
//! [`OperatorId`]: elide_core::redaction::OperatorId

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::OperatorIdSchema;

/// SHA-2 variant for the [`TextRedaction::Hash`] operator.
///
/// Spec mirror of [`elide::redaction::operators::HashAlgorithm`]; the
/// engine maps between the two at compile time. Local copy so the
/// wire format owns its vocabulary (the upstream enum is not
/// serialisable today).
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

/// Operator spec a `redact` text rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextRedaction {
    /// Substitute the span with a fixed template. Supports
    /// `{label}` / `{value}` / `{coref}` placeholders.
    Replace {
        /// Template string. Default `[{label}]`.
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
    Erase,
    /// Pass the value through unchanged.
    Keep,
    /// Look up a deployment-registered custom operator by id.
    Custom {
        /// Id under which the operator was registered.
        id: OperatorIdSchema,
    },
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
