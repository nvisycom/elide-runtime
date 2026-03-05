//! Flat redaction method identifiers (no configuration payload).
//!
//! Each [`TextRedactionMethod`], [`ImageRedactionMethod`], and
//! [`AudioRedactionMethod`] names the *kind* of redaction to apply
//! without carrying method-specific parameters. These are the types an
//! LLM agent returns when recommending a redaction strategy; downstream
//! code maps them into the full [`TextRedactionInput`](super::TextRedactionInput),
//! [`ImageRedactionInput`](super::ImageRedactionInput), or
//! [`AudioRedactionInput`](super::AudioRedactionInput) with appropriate
//! defaults.

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::Display;

/// Text/tabular redaction method.
///
/// | Variant | Effect |
/// |---|---|
/// | `Mask` | Replace characters with a fixed mask character |
/// | `Replace` | Substitute with a type-appropriate placeholder |
/// | `Hash` | Replace with a one-way hash |
/// | `Encrypt` | Encrypt the value (recoverable with key) |
/// | `Remove` | Delete the value entirely |
/// | `Generate` | Replace with a realistic generated value |
/// | `Pseudonymize` | Replace with a consistent pseudonym |
/// | `Tokenize` | Replace with a vault-backed reversible token |
/// | `Aggregate` | Aggregate into a range or bucket |
/// | `Generalize` | Generalize to a less precise value |
/// | `DateShift` | Shift dates by a consistent offset |
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Display,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TextRedactionMethod {
    /// Replace characters with a mask character (e.g. `***`).
    Mask,
    /// Substitute with a fixed placeholder (e.g. `[EMAIL]`).
    Replace,
    /// Replace with a one-way hash.
    Hash,
    /// Encrypt the value; recoverable with a referenced key.
    Encrypt,
    /// Remove the value entirely.
    Remove,
    /// Replace with a realistically generated value.
    Generate,
    /// Replace with a consistent pseudonym.
    Pseudonymize,
    /// Replace with a vault-backed reversible token.
    Tokenize,
    /// Aggregate into a range or bucket.
    Aggregate,
    /// Generalize to a less precise value.
    Generalize,
    /// Shift dates by a consistent offset.
    DateShift,
}

/// Image redaction method.
///
/// | Variant | Effect |
/// |---|---|
/// | `Blur` | Apply a gaussian blur over the region |
/// | `Block` | Overlay an opaque rectangle |
/// | `Pixelate` | Apply pixelation / mosaic effect |
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Display,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ImageRedactionMethod {
    /// Apply a gaussian blur over the region.
    Blur,
    /// Overlay an opaque rectangle.
    Block,
    /// Apply pixelation / mosaic effect.
    Pixelate,
}

/// Audio redaction method.
///
/// | Variant | Effect |
/// |---|---|
/// | `Silence` | Replace audio segment with silence |
/// | `Remove` | Remove the segment entirely |
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Display,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AudioRedactionMethod {
    /// Replace audio segment with silence.
    Silence,
    /// Remove the segment entirely.
    Remove,
}

/// Unified redaction method across all modalities.
///
/// Mirrors the structure of [`RedactionInput`](super::RedactionInput) but
/// carries only the method name — no configuration payload.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    From,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
pub enum RedactionMethod {
    /// Text/tabular redaction method.
    Text(TextRedactionMethod),
    /// Image redaction method.
    Image(ImageRedactionMethod),
    /// Audio redaction method.
    Audio(AudioRedactionMethod),
}
