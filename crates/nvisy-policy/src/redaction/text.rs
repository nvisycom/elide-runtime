//! [`TextRedaction`]: the operator spec a text-modality policy rule
//! carries.
//!
//! Each variant mirrors an elide built-in operator the engine
//! constructs at apply time:
//!
//! - [`TextRedaction::Erase`] → `elide::redaction::operators::Erase`
//! - [`TextRedaction::Keep`] → `elide::redaction::operators::Keep`
//! - [`TextRedaction::Mask`] → `elide::redaction::operators::Mask`
//! - [`TextRedaction::Replace`] → `elide::redaction::operators::Replace`
//! - [`TextRedaction::Hash`] → `elide::redaction::operators::Sha2Hash`
//! - [`TextRedaction::Fake`] → `elide_fake::Fake` (locale-aware
//!   surrogate values; the engine wires a `Replace`-with-default-
//!   template fallback for labels outside the core PII catalogue)
//! - [`TextRedaction::Pseudonymize`] →
//!   `elide::redaction::operators::Pseudonymize` (engine wires a
//!   per-request in-memory [`Vault`] and the default [`RandomToken`]
//!   [`Generator`])
//! - [`TextRedaction::Encrypt`] →
//!   `elide::redaction::operators::AesEncrypt` (engine wires the
//!   per-tenant AES [`KeyProvider`])
//! - [`TextRedaction::HmacHash`] →
//!   `elide::redaction::operators::HmacHash` (engine wires the
//!   per-tenant [`KeyProvider`]; distinct from [`Hash`] because the
//!   HMAC key stays secret, blocking offline dictionary attacks a
//!   public salt does not — the PCI DSS v4.0.1 §3.5.1 "keyed hash"
//!   posture)
//! - [`TextRedaction::Truncate`] →
//!   `elide::redaction::operators::Truncate` (physically drop the
//!   middle of the value; distinct from [`Mask`] which preserves
//!   length — the PCI DSS §3.5.1 truncation posture for stored PAN)
//! - [`TextRedaction::Clamp`] →
//!   `elide::redaction::operators::Clamp` (numeric ceiling/floor →
//!   bucket label; the HIPAA §164.514(b)(2)(i)(C) age-cap posture)
//! - [`TextRedaction::GeneralizeDate`] →
//!   `elide::redaction::operators::GeneralizeDate` (reduce date
//!   granularity; the HIPAA §164.514(b)(2)(i)(C) date-generalization
//!   posture)
//!
//! [`Clamp`] and [`GeneralizeDate`] each carry an optional
//! `fallback` field that runs when the entity value isn't the
//! operator's shape (a non-numeric age, a free-text date). The
//! fallback is one of the four terminal operators — [`Erase`],
//! [`Keep`], [`Replace`], [`Mask`] — so the engine builds a
//! concrete `elide::WithFallback<primary, fallback>` without
//! type erasure. `None` erases (elide's baked-in default).
//!
//! No `Custom` escape hatch: every operator the wire format admits
//! is predefined. New built-ins land in elide first, then surface
//! here as new variants.
//!
//! [`Clamp`]: TextRedaction::Clamp
//! [`Erase`]: TextRedaction::Erase
//! [`GeneralizeDate`]: TextRedaction::GeneralizeDate
//! [`Hash`]: TextRedaction::Hash
//! [`Keep`]: TextRedaction::Keep
//! [`Mask`]: TextRedaction::Mask
//! [`Replace`]: TextRedaction::Replace
//! [`Generator`]: https://docs.rs/elide/latest/elide/redaction/generator/trait.Generator.html
//! [`KeyProvider`]: https://docs.rs/elide/latest/elide/redaction/operators/trait.KeyProvider.html
//! [`RandomToken`]: https://docs.rs/elide/latest/elide/redaction/generator/struct.RandomToken.html
//! [`Vault`]: https://docs.rs/elide/latest/elide/redaction/vault/trait.Vault.html

use std::collections::HashMap;

use elide_core::primitive::{LanguageTag, LocalizedText};
use elide_redaction::operators::Clamp;
pub use elide_redaction::operators::{DateGranularity, DateStyle, Sha2Algorithm};
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Text a [`TextRedaction::Clamp`] emits for out-of-range values.
///
/// Three forms, deserialized untagged so callers can pick the
/// terser one for the case at hand:
///
/// - **Plain string** (`"90 or older"`) — English-only shorthand.
/// - **Localized map** (`{"en": "90 or older", "fr": "90 ou plus"}`)
///   — one entry per language the deployment ships; missing
///   locales fall back to English at render time.
/// - **Format template** (`{"format": "{n} or older"}`) — the
///   engine substitutes `{n}` for the threshold, so a ceiling of
///   `90` renders `"90 or older"` without the caller repeating the
///   number. Same rendering in every language.
///
/// Round-trips through serde as whichever form the caller wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ClampBucket {
    /// Plain English-only string.
    Plain(String),
    /// Format template — the engine substitutes `{n}` for the
    /// threshold. Same rendering in every language.
    Format {
        /// The template string. `{n}` is substituted; other text
        /// is literal.
        format: String,
    },
    /// Localized map, keyed by BCP-47 language tag.
    Localized(HashMap<LanguageTag, String>),
}

impl ClampBucket {
    /// Attach this bucket as the ceiling of `clamp` at `threshold`,
    /// returning the extended [`Clamp`].
    #[must_use]
    pub fn attach_ceiling(&self, clamp: Clamp, threshold: f64) -> Clamp {
        match self {
            Self::Plain(text) => clamp.with_ceiling(threshold, text.clone()),
            Self::Format { format } => clamp.with_ceiling_fmt(threshold, format.clone()),
            Self::Localized(map) => clamp.with_ceiling(threshold, localized(map)),
        }
    }

    /// Attach this bucket as the floor of `clamp` at `threshold`,
    /// returning the extended [`Clamp`].
    #[must_use]
    pub fn attach_floor(&self, clamp: Clamp, threshold: f64) -> Clamp {
        match self {
            Self::Plain(text) => clamp.with_floor(threshold, text.clone()),
            Self::Format { format } => clamp.with_floor_fmt(threshold, format.clone()),
            Self::Localized(map) => clamp.with_floor(threshold, localized(map)),
        }
    }
}

fn localized(map: &HashMap<LanguageTag, String>) -> LocalizedText<HipStr<'static>> {
    map.iter()
        .map(|(lang, value)| (lang.clone(), HipStr::from(value.as_str())))
        .collect()
}

/// Fallback operator that runs when a declinable primary
/// ([`TextRedaction::Clamp`], [`TextRedaction::GeneralizeDate`])
/// doesn't apply to the entity value.
///
/// The four operators that always apply and produce a deterministic
/// output without needing engine-side infrastructure (no key
/// provider, no vault). Enough to satisfy every regulatory
/// pattern I know: `Clamp/GeneralizeDate → Erase` (the safe
/// default), `→ Replace { template }` (an explicit placeholder),
/// or `→ Mask` / `→ Keep` on the rare occasion those fit.
///
/// Absent from the primary's spec, elide's baked-in default is
/// [`Erase`] — a bare [`TextRedaction::Clamp`] without a `fallback`
/// erases values that aren't numeric.
///
/// [`Erase`]: TerminalFallback::Erase
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalFallback {
    /// See [`TextRedaction::Erase`].
    Erase,
    /// See [`TextRedaction::Keep`].
    Keep,
    /// See [`TextRedaction::Replace`].
    Replace {
        /// Template string. Default `[{label}]`.
        #[serde(default = "default_replace_template")]
        template: String,
    },
    /// See [`TextRedaction::Mask`].
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
}

/// Operator spec a `redact` text rule carries.
#[derive(Debug, Clone, PartialEq)]
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
        algorithm: Sha2Algorithm,
        /// Salt prepended to the value before hashing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        salt: Option<String>,
    },
    /// Swap the matched span for a locale-aware fake value. Picks
    /// the locale from the entity's BCP-47 `language` tag, falling
    /// back to `default_language` (English unless overridden) when
    /// the entity carries none. Coreferent mentions of the same
    /// real-world entity collapse to the same surrogate within a
    /// run. Labels outside the built-in PII catalogue fall through
    /// to [`Replace`] with `fallback_template`.
    ///
    /// [`Replace`]: TextRedaction::Replace
    Fake {
        /// BCP-47 tag used when the entity carries no language of
        /// its own. Defaults to English (`"en"`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_language: Option<LanguageTag>,
        /// Seed mixed into per-entity RNG state. Two runs with the
        /// same seed and the same input entities produce the same
        /// surrogates. Defaults to `0`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seed: Option<u64>,
        /// Template used for entity labels outside the built-in
        /// PII catalogue (which `Fake` can't generate for).
        /// Supports the same `{label}` / `{value}` / `{coref}`
        /// placeholders as [`Replace`]. Defaults to `[{label}]`.
        ///
        /// [`Replace`]: TextRedaction::Replace
        #[serde(default = "default_replace_template")]
        fallback_template: String,
    },
    /// Vault-backed pseudonym: every mention of the same entity
    /// reads the same surrogate. The engine wires a per-request
    /// vault + the default [`RandomToken`] generator.
    ///
    /// [`RandomToken`]: https://docs.rs/elide/latest/elide/redaction/generator/struct.RandomToken.html
    Pseudonymize,
    /// Reversible AES-256-GCM ciphertext. The engine supplies the
    /// per-tenant AES key at construction so raw key material never
    /// lives in serialised policy. Requires the engine to have a
    /// key provider wired via `Engine::with_key_provider`.
    Encrypt,
    /// Keyed HMAC-SHA-2 digest. The key stays secret, so an
    /// attacker who obtains the redacted output cannot enumerate
    /// a small input space without the key — the PCI DSS v4.0.1
    /// §3.5.1 "keyed hash" posture. Distinct from [`Hash`], whose
    /// salt is public. Requires the engine to have a key provider
    /// wired via `Engine::with_key_provider`.
    ///
    /// [`Hash`]: TextRedaction::Hash
    HmacHash {
        /// HMAC-SHA-256 (default) or HMAC-SHA-512.
        #[serde(default)]
        algorithm: Sha2Algorithm,
    },
    /// Physically remove the middle of the value, keeping a
    /// leading and/or trailing run of characters. Unlike [`Mask`]
    /// this *shortens* the string — the dropped characters leave
    /// no placeholder. The PCI DSS §3.5.1 truncation posture for
    /// stored PAN: `Truncate { keep_prefix: 6, keep_suffix: 4 }`
    /// on `"4111111111111234"` yields `"4111111234"` (10 chars),
    /// where the analogous [`Mask`] would yield `"411111******1234"`
    /// (16 chars, length preserved).
    ///
    /// A configuration whose kept regions cover (or overlap) the
    /// whole value is rejected at apply time; the operator errors
    /// rather than silently pass through.
    ///
    /// [`Mask`]: TextRedaction::Mask
    Truncate {
        /// Characters to keep at the start of the value.
        #[serde(default, skip_serializing_if = "is_zero")]
        keep_prefix: usize,
        /// Characters to keep at the end of the value.
        #[serde(default, skip_serializing_if = "is_zero")]
        keep_suffix: usize,
    },
    /// Collapse a numeric value at or above `ceiling` (or at or
    /// below `floor`) into a bucket label; values in the middle
    /// pass through unchanged. The HIPAA §164.514(b)(2)(i)(C) age
    /// posture: everyone ≥90 aggregates into `"90 or older"`,
    /// while a 73-year-old stays `"73"`.
    ///
    /// Values that don't parse as finite numbers are *declined*.
    /// `fallback` names what runs on a declined value; when
    /// absent, elide's baked-in default is [`Erase`] (the safe
    /// posture — a non-numeric age never survives redaction).
    ///
    /// [`Erase`]: TextRedaction::Erase
    Clamp {
        /// Threshold at or above which values collapse to
        /// `ceiling_bucket`. `None` disables the ceiling.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ceiling: Option<f64>,
        /// Bucket label for values at or above `ceiling`. Required
        /// when `ceiling` is set; ignored otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ceiling_bucket: Option<ClampBucket>,
        /// Threshold at or below which values collapse to
        /// `floor_bucket`. `None` disables the floor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        floor: Option<f64>,
        /// Bucket label for values at or below `floor`. Required
        /// when `floor` is set; ignored otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        floor_bucket: Option<ClampBucket>,
        /// Operator that runs when the entity value isn't a
        /// finite number. `None` erases (elide's default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<TerminalFallback>,
    },
    /// Reduce a date/timestamp to a coarser granularity. The
    /// HIPAA §164.514(b)(2)(i)(C) date-generalization posture:
    /// dates directly related to an individual reduced to the
    /// year, preserving cohort-level analytics.
    ///
    /// Values that don't parse as dates are *declined*. `fallback`
    /// names what runs on a declined value; when absent, elide's
    /// baked-in default is [`Erase`].
    ///
    /// [`Erase`]: TextRedaction::Erase
    GeneralizeDate {
        /// Coarseness of the output. Default `Year`.
        #[serde(default)]
        granularity: DateGranularity,
        /// Which input convention to accept. Default `Iso`.
        #[serde(default)]
        style: DateStyle,
        /// Operator that runs when the entity value isn't a
        /// parseable date. `None` erases (elide's default).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<TerminalFallback>,
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
