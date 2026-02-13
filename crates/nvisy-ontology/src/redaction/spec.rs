//! Data-carrying redaction specifications submitted to the engine.
//!
//! A [`RedactionSpec`] describes *how* to redact — which method to apply and
//! the configuration parameters it needs (mask char, blur sigma, encryption
//! key id, etc.). Used on [`PolicyRule`](crate::policy::PolicyRule) and
//! [`Policy`](crate::policy::Policy).

use derive_more::From;
use serde::{Deserialize, Serialize};

use super::method::{
    AudioRedactionMethod, ImageRedactionMethod, RedactionMethod, TextRedactionMethod,
};

/// Text redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextRedactionSpec {
    /// Replace characters with a mask character.
    Mask {
        /// Character used for masking (default `'*'`).
        #[serde(default = "default_mask_char")]
        mask_char: char,
    },
    /// Substitute with a fixed placeholder string.
    Replace {
        /// Template for the replacement (supports `{entityType}`, `{category}`, `{value}`).
        #[serde(default)]
        placeholder: String,
    },
    /// Replace with a one-way hash.
    Hash,
    /// Encrypt the value; recoverable with the referenced key.
    Encrypt {
        /// Identifier of the encryption key to use.
        key_id: String,
    },
    /// Remove the value entirely.
    Remove,
    /// Replace with a synthetically generated value.
    Synthesize,
    /// Replace with a consistent pseudonym.
    Pseudonymize,
    /// Replace with a vault-backed reversible token.
    Tokenize {
        /// Identifier of the token vault.
        #[serde(default)]
        vault_id: Option<String>,
    },
    /// Aggregate into a range or bucket.
    Aggregate,
    /// Generalize to a less precise value.
    Generalize {
        /// Generalization level (1 = city, 2 = state, etc.).
        #[serde(default)]
        level: Option<u32>,
    },
    /// Shift dates by a consistent offset.
    DateShift {
        /// Fixed offset in days (0 = engine picks a random offset).
        #[serde(default)]
        offset_days: i64,
    },
}

fn default_mask_char() -> char {
    '*'
}

/// Image redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageRedactionSpec {
    /// Apply a gaussian blur.
    Blur {
        /// Blur sigma value.
        #[serde(default = "default_sigma")]
        sigma: f32,
    },
    /// Overlay an opaque block.
    Block {
        /// RGBA color for the block.
        #[serde(default = "default_block_color")]
        color: [u8; 4],
    },
    /// Apply pixelation (mosaic).
    Pixelate {
        /// Pixel block size.
        #[serde(default = "default_block_size")]
        block_size: u32,
    },
    /// Replace with a synthetic region.
    Synthesize,
}

fn default_sigma() -> f32 {
    15.0
}
fn default_block_color() -> [u8; 4] {
    [0, 0, 0, 255]
}
fn default_block_size() -> u32 {
    10
}

/// Audio redaction specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionSpec {
    /// Replace with silence.
    Silence,
    /// Remove the segment entirely.
    Remove,
    /// Replace with synthetic audio.
    Synthesize,
}

/// Unified redaction specification submitted to the engine.
///
/// Carries the method to apply and its configuration parameters.
/// Used on [`PolicyRule`](crate::policy::PolicyRule) and
/// [`Policy`](crate::policy::Policy).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RedactionSpec {
    /// Text/tabular redaction specification.
    Text(TextRedactionSpec),
    /// Image/video redaction specification.
    Image(ImageRedactionSpec),
    /// Audio redaction specification.
    Audio(AudioRedactionSpec),
}

impl RedactionSpec {
    /// Returns the [`RedactionMethod`] tag this spec corresponds to.
    pub fn method(&self) -> RedactionMethod {
        match self {
            Self::Text(t) => RedactionMethod::Text(match t {
                TextRedactionSpec::Mask { .. } => TextRedactionMethod::Mask,
                TextRedactionSpec::Replace { .. } => TextRedactionMethod::Replace,
                TextRedactionSpec::Hash => TextRedactionMethod::Hash,
                TextRedactionSpec::Encrypt { .. } => TextRedactionMethod::Encrypt,
                TextRedactionSpec::Remove => TextRedactionMethod::Remove,
                TextRedactionSpec::Synthesize => TextRedactionMethod::Synthesize,
                TextRedactionSpec::Pseudonymize => TextRedactionMethod::Pseudonymize,
                TextRedactionSpec::Tokenize { .. } => TextRedactionMethod::Tokenize,
                TextRedactionSpec::Aggregate => TextRedactionMethod::Aggregate,
                TextRedactionSpec::Generalize { .. } => TextRedactionMethod::Generalize,
                TextRedactionSpec::DateShift { .. } => TextRedactionMethod::DateShift,
            }),
            Self::Image(i) => RedactionMethod::Image(match i {
                ImageRedactionSpec::Blur { .. } => ImageRedactionMethod::Blur,
                ImageRedactionSpec::Block { .. } => ImageRedactionMethod::Block,
                ImageRedactionSpec::Pixelate { .. } => ImageRedactionMethod::Pixelate,
                ImageRedactionSpec::Synthesize => ImageRedactionMethod::Synthesize,
            }),
            Self::Audio(a) => RedactionMethod::Audio(match a {
                AudioRedactionSpec::Silence => AudioRedactionMethod::Silence,
                AudioRedactionSpec::Remove => AudioRedactionMethod::Remove,
                AudioRedactionSpec::Synthesize => AudioRedactionMethod::Synthesize,
            }),
        }
    }
}
