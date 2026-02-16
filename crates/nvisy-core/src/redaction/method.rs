//! Plain-tag redaction method enums.
//!
//! These are lightweight identifiers that name a redaction algorithm without
//! carrying any configuration data. For a data-carrying request see
//! [`RedactionSpec`](super::RedactionSpec); for a data-carrying result see
//! [`RedactionOutput`](super::RedactionOutput).

use derive_more::From;
use serde::{Deserialize, Serialize};

/// Redaction strategies for text and tabular content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TextRedactionMethod {
    /// Replace characters with a mask character (e.g. `***-**-1234`).
    Mask,
    /// Substitute with a fixed placeholder string.
    Replace,
    /// Replace with a one-way hash of the original value.
    Hash,
    /// Encrypt the value so it can be recovered later with a key.
    Encrypt,
    /// Remove the value entirely from the output.
    Remove,
    /// Replace with a synthetically generated realistic value.
    Synthesize,
    /// Replace with a consistent pseudonym across the document.
    Pseudonymize,
    /// Replace with a vault-backed reversible token (e.g. `USER_001`).
    Tokenize,
    /// Aggregate value into a range or bucket (e.g. age 34 → 30-39).
    Aggregate,
    /// Generalize to a less precise value (e.g. street → city → country).
    Generalize,
    /// Shift dates by a random but consistent offset, preserving intervals.
    DateShift,
}

/// Redaction strategies for image and video regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ImageRedactionMethod {
    /// Apply a gaussian blur to the region.
    Blur,
    /// Overlay an opaque block over the region.
    Block,
    /// Apply pixelation to the region (mosaic effect).
    Pixelate,
    /// Replace with a synthetically generated region.
    Synthesize,
}

/// Redaction strategies for audio segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AudioRedactionMethod {
    /// Replace the audio segment with silence.
    Silence,
    /// Remove the audio segment entirely.
    Remove,
    /// Replace with synthetically generated audio.
    Synthesize,
}

/// Unified redaction strategy tag that wraps modality-specific methods.
///
/// This is a lightweight identifier — it names the algorithm but carries no
/// configuration data. For a data-carrying request use [`RedactionSpec`](super::RedactionSpec);
/// for a data-carrying result use [`RedactionOutput`](super::RedactionOutput).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, From, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedactionMethod {
    /// Text/tabular redaction strategy.
    Text(TextRedactionMethod),
    /// Image/video redaction strategy.
    Image(ImageRedactionMethod),
    /// Audio redaction strategy.
    Audio(AudioRedactionMethod),
}

impl RedactionMethod {
    /// Returns the text redaction method if this is a text variant.
    pub fn as_text(&self) -> Option<TextRedactionMethod> {
        match self {
            Self::Text(m) => Some(*m),
            _ => None,
        }
    }

    /// Returns the image redaction method if this is an image variant.
    pub fn as_image(&self) -> Option<ImageRedactionMethod> {
        match self {
            Self::Image(m) => Some(*m),
            _ => None,
        }
    }

    /// Returns the audio redaction method if this is an audio variant.
    pub fn as_audio(&self) -> Option<AudioRedactionMethod> {
        match self {
            Self::Audio(m) => Some(*m),
            _ => None,
        }
    }
}
