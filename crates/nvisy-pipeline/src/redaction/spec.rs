//! Data-carrying redaction specifications submitted to the engine.
//!
//! A [`RedactionSpec`] describes *how* to redact — which method to apply and
//! the configuration parameters it needs (mask char, blur sigma, encryption
//! key id, etc.).

use derive_more::From;
use serde::{Deserialize, Serialize};

/// Text redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
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

/// Default mask character for text redaction.
pub const DEFAULT_MASK_CHAR: char = '*';

/// Default gaussian blur sigma value.
pub const DEFAULT_BLUR_SIGMA: f32 = 15.0;

/// Default RGBA color for block overlays (opaque black).
pub const DEFAULT_BLOCK_COLOR: [u8; 4] = [0, 0, 0, 255];

/// Default pixel block size for pixelation/mosaic.
pub const DEFAULT_PIXELATE_BLOCK_SIZE: u32 = 10;

fn default_mask_char() -> char {
    DEFAULT_MASK_CHAR
}

/// Image redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
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
    DEFAULT_BLUR_SIGMA
}
fn default_block_color() -> [u8; 4] {
    DEFAULT_BLOCK_COLOR
}
fn default_block_size() -> u32 {
    DEFAULT_PIXELATE_BLOCK_SIZE
}

/// Audio redaction specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionSpec {
    /// Text/tabular redaction specification.
    Text(TextRedactionSpec),
    /// Image/video redaction specification.
    Image(ImageRedactionSpec),
    /// Audio redaction specification.
    Audio(AudioRedactionSpec),
}
