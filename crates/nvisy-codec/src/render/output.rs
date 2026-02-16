//! Data-carrying redaction output enums recording what was done.
//!
//! A [`RedactionOutput`] records the method that was applied and its result
//! data (replacement string, ciphertext, blur sigma, etc.).

use derive_more::From;
use serde::{Deserialize, Serialize};

use nvisy_core::redaction::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};

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

/// Image redaction output — records the method used and its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
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
#[derive(schemars::JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionOutput {
    /// Segment replaced with silence.
    Silence,
    /// Segment removed entirely.
    Remove,
    /// Segment replaced with synthetic audio.
    Synthesize,
}

impl TextRedactionOutput {
    /// Returns the [`TextRedactionMethod`] tag this output corresponds to.
    pub fn method(&self) -> TextRedactionMethod {
        match self {
            Self::Mask { .. } => TextRedactionMethod::Mask,
            Self::Replace { .. } => TextRedactionMethod::Replace,
            Self::Hash { .. } => TextRedactionMethod::Hash,
            Self::Encrypt { .. } => TextRedactionMethod::Encrypt,
            Self::Remove => TextRedactionMethod::Remove,
            Self::Synthesize { .. } => TextRedactionMethod::Synthesize,
            Self::Pseudonymize { .. } => TextRedactionMethod::Pseudonymize,
            Self::Tokenize { .. } => TextRedactionMethod::Tokenize,
            Self::Aggregate { .. } => TextRedactionMethod::Aggregate,
            Self::Generalize { .. } => TextRedactionMethod::Generalize,
            Self::DateShift { .. } => TextRedactionMethod::DateShift,
        }
    }

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

impl ImageRedactionOutput {
    /// Returns the [`ImageRedactionMethod`] tag this output corresponds to.
    pub fn method(&self) -> ImageRedactionMethod {
        match self {
            Self::Blur { .. } => ImageRedactionMethod::Blur,
            Self::Block { .. } => ImageRedactionMethod::Block,
            Self::Pixelate { .. } => ImageRedactionMethod::Pixelate,
            Self::Synthesize => ImageRedactionMethod::Synthesize,
        }
    }
}

impl AudioRedactionOutput {
    /// Returns the [`AudioRedactionMethod`] tag this output corresponds to.
    pub fn method(&self) -> AudioRedactionMethod {
        match self {
            Self::Silence => AudioRedactionMethod::Silence,
            Self::Remove => AudioRedactionMethod::Remove,
            Self::Synthesize => AudioRedactionMethod::Synthesize,
        }
    }
}

/// Unified redaction output that wraps modality-specific output variants.
///
/// Carries method-specific result data (replacement strings, ciphertext,
/// blur sigma, etc.).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
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
            Self::Text(t) => t.replacement_value(),
            Self::Image(_) | Self::Audio(_) => None,
        }
    }

    /// Returns the [`RedactionMethod`] tag this output corresponds to.
    pub fn method(&self) -> RedactionMethod {
        match self {
            Self::Text(t) => RedactionMethod::Text(t.method()),
            Self::Image(i) => RedactionMethod::Image(i.method()),
            Self::Audio(a) => RedactionMethod::Audio(a.method()),
        }
    }
}
