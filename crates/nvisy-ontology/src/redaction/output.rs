//! Data-carrying redaction output enums recording what was done.
//!
//! A [`RedactionOutput`] records the method that was applied and its result
//! data (replacement string, ciphertext, blur sigma, etc.). Stored on
//! [`Redaction`](super::Redaction).

use derive_more::From;
use serde::{Deserialize, Serialize};

use super::method::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};

/// Text redaction output — records the method used and its replacement data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
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

/// Image redaction output — records the method used and its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageRedactionOutput {
    /// Gaussian blur applied to the region.
    Blur { sigma: f32 },
    /// Opaque block overlay on the region.
    Block { color: [u8; 4] },
    /// Pixelation (mosaic) applied to the region.
    Pixelate { block_size: u32 },
    /// Region replaced with a synthetic image.
    Synthesize,
}

/// Audio redaction output — records the method used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionOutput {
    /// Segment replaced with silence.
    Silence,
    /// Segment removed entirely.
    Remove,
    /// Segment replaced with synthetic audio.
    Synthesize,
}

/// Unified redaction output that wraps modality-specific output variants.
///
/// Carries method-specific result data (replacement strings, ciphertext,
/// blur sigma, etc.). Stored on [`Redaction`](super::Redaction).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RedactionOutput {
    /// Text/tabular redaction output.
    Text(TextRedactionOutput),
    /// Image/video redaction output.
    Image(ImageRedactionOutput),
    /// Audio redaction output.
    Audio(AudioRedactionOutput),
}

impl RedactionOutput {
    /// Returns the text replacement string, regardless of specific method.
    ///
    /// Used by apply actions that just need to know "what string goes here".
    /// Returns `None` for image and audio outputs, or text `Remove`.
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Text(t) => match t {
                TextRedactionOutput::Mask { replacement, .. } => Some(replacement),
                TextRedactionOutput::Replace { replacement } => Some(replacement),
                TextRedactionOutput::Hash { hash_value } => Some(hash_value),
                TextRedactionOutput::Encrypt { ciphertext, .. } => Some(ciphertext),
                TextRedactionOutput::Remove => None,
                TextRedactionOutput::Synthesize { replacement } => Some(replacement),
                TextRedactionOutput::Pseudonymize { pseudonym } => Some(pseudonym),
                TextRedactionOutput::Tokenize { token, .. } => Some(token),
                TextRedactionOutput::Aggregate { replacement } => Some(replacement),
                TextRedactionOutput::Generalize { replacement, .. } => Some(replacement),
                TextRedactionOutput::DateShift { replacement, .. } => Some(replacement),
            },
            Self::Image(_) | Self::Audio(_) => None,
        }
    }

    /// Returns the [`RedactionMethod`] tag this output corresponds to.
    pub fn method(&self) -> RedactionMethod {
        match self {
            Self::Text(t) => RedactionMethod::Text(match t {
                TextRedactionOutput::Mask { .. } => TextRedactionMethod::Mask,
                TextRedactionOutput::Replace { .. } => TextRedactionMethod::Replace,
                TextRedactionOutput::Hash { .. } => TextRedactionMethod::Hash,
                TextRedactionOutput::Encrypt { .. } => TextRedactionMethod::Encrypt,
                TextRedactionOutput::Remove => TextRedactionMethod::Remove,
                TextRedactionOutput::Synthesize { .. } => TextRedactionMethod::Synthesize,
                TextRedactionOutput::Pseudonymize { .. } => TextRedactionMethod::Pseudonymize,
                TextRedactionOutput::Tokenize { .. } => TextRedactionMethod::Tokenize,
                TextRedactionOutput::Aggregate { .. } => TextRedactionMethod::Aggregate,
                TextRedactionOutput::Generalize { .. } => TextRedactionMethod::Generalize,
                TextRedactionOutput::DateShift { .. } => TextRedactionMethod::DateShift,
            }),
            Self::Image(i) => RedactionMethod::Image(match i {
                ImageRedactionOutput::Blur { .. } => ImageRedactionMethod::Blur,
                ImageRedactionOutput::Block { .. } => ImageRedactionMethod::Block,
                ImageRedactionOutput::Pixelate { .. } => ImageRedactionMethod::Pixelate,
                ImageRedactionOutput::Synthesize => ImageRedactionMethod::Synthesize,
            }),
            Self::Audio(a) => RedactionMethod::Audio(match a {
                AudioRedactionOutput::Silence => AudioRedactionMethod::Silence,
                AudioRedactionOutput::Remove => AudioRedactionMethod::Remove,
                AudioRedactionOutput::Synthesize => AudioRedactionMethod::Synthesize,
            }),
        }
    }
}
