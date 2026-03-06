//! Redaction input types: configuration-carrying specifications submitted
//! to the redaction engine, and the [`RedactorInput`] context struct
//! passed to LLM agents.

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{EntityCategory, EntityKind};

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

/// Text redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextRedactionInput {
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
    /// Shift dates by a consistent offset.
    DateShift {
        /// Fixed offset in days (0 = engine picks a random offset).
        #[serde(default)]
        offset_days: i64,
    },
}

/// Image redaction specification with method-specific configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageRedactionInput {
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

/// Audio redaction specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioRedactionInput {
    /// Replace with silence.
    Silence,
    /// Remove the segment entirely.
    Remove,
}

/// Unified redaction specification submitted to the engine.
///
/// Carries the method to apply and its configuration parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(From, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionInput {
    /// Text/tabular redaction specification.
    Text(TextRedactionInput),
    /// Image redaction specification.
    Image(ImageRedactionInput),
    /// Audio redaction specification.
    Audio(AudioRedactionInput),
}

/// Entity passed to a redactor agent for decision-making.
///
/// Contains the detected entity's classification, matched value, confidence,
/// and byte offsets in the source text. The redactor uses this context to
/// choose an appropriate [`RedactionMethod`](super::RedactionMethod).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactorInput {
    /// Specific entity type (e.g. `EmailAddress`, `GovernmentId`).
    pub entity_type: EntityKind,
    /// Broad classification (e.g. `Pii`, `Financial`).
    pub category: EntityCategory,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0 -- 1.0).
    pub confidence: f64,
    /// Start byte offset in the input text.
    pub start_offset: usize,
    /// End byte offset in the input text.
    pub end_offset: usize,
}
