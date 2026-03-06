//! Redaction strategies for text, image, and audio modalities.
//!
//! Each per-modality strategy ([`TextRedactionStrategy`],
//! [`ImageRedactionStrategy`], [`AudioRedactionStrategy`]) pairs a redaction
//! method with its configuration parameters. [`RedactionStrategy`] unifies
//! them under a single tagged enum for policy rules and pipeline decisions.

mod audio;
mod image;
mod text;

pub use audio::AudioRedactionStrategy;
pub use image::{
    ImageRedactionStrategy, DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_PIXELATE_BLOCK_SIZE,
};
pub use text::{TextRedactionStrategy, DEFAULT_MASK_CHAR};

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
