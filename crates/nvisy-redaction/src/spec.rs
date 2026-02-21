//! Unified redaction specification.
//!
//! Re-exports modality-specific specs from their submodules and defines
//! the top-level [`RedactionSpec`] wrapper.

use derive_more::From;
use serde::{Deserialize, Serialize};

pub use super::text::spec::{TextRedactionSpec, DEFAULT_MASK_CHAR};
pub use super::image::spec::{
    ImageRedactionSpec, DEFAULT_BLOCK_COLOR, DEFAULT_BLUR_SIGMA, DEFAULT_PIXELATE_BLOCK_SIZE,
};
pub use super::audio::spec::AudioRedactionSpec;

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
