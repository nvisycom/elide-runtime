//! Lightweight redaction method identifiers (no configuration payload).
//!
//! Each variant names the *kind* of redaction to apply without carrying
//! method-specific parameters. LLM agents return these when recommending
//! a redaction strategy; downstream code maps them into the corresponding
//! [`RedactionStrategy`](super::RedactionStrategy) with appropriate defaults.

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::Display;

/// Text and tabular redaction method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, Serialize, Deserialize, JsonSchema)]
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
}

/// Image redaction method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, Serialize, Deserialize, JsonSchema)]
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
/// Wraps a per-modality method variant. Carries only the method name —
/// no configuration payload. See [`RedactionStrategy`](super::RedactionStrategy)
/// for the configuration-carrying counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(From, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionMethod {
    /// Text/tabular redaction method.
    Text(TextRedactionMethod),
    /// Image redaction method.
    Image(ImageRedactionMethod),
    /// Audio redaction method.
    Audio(AudioRedactionMethod),
}
