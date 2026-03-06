//! Configuration-carrying redaction strategies.
//!
//! Each variant pairs a redaction method with its parameters (mask
//! character, blur sigma, encryption key, etc.). Policy rules store
//! these, and the redaction engine matches on them to apply transforms.

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
fn default_sigma() -> f32 {
    DEFAULT_BLUR_SIGMA
}
fn default_block_color() -> [u8; 4] {
    DEFAULT_BLOCK_COLOR
}
fn default_block_size() -> u32 {
    DEFAULT_PIXELATE_BLOCK_SIZE
}

/// Text redaction strategy with method-specific configuration.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextRedactionStrategy {
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
    /// Replace with a realistically generated value.
    Generate,
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
}

/// Image redaction strategy with method-specific configuration.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageRedactionStrategy {
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
}

/// Audio redaction strategy.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionStrategy {
    /// Replace with silence.
    Silence,
    /// Remove the segment entirely.
    Remove,
}

/// Unified redaction strategy across all modalities.
///
/// Wraps a per-modality strategy variant carrying the method and its
/// configuration parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(From, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionStrategy {
    /// Text/tabular redaction strategy.
    Text(TextRedactionStrategy),
    /// Image redaction strategy.
    Image(ImageRedactionStrategy),
    /// Audio redaction strategy.
    Audio(AudioRedactionStrategy),
}
