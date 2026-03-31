//! Redaction strategies for text, image, and audio modalities.
//!
//! Each per-modality strategy ([`TextStrategy`], [`ImageStrategy`],
//! [`AudioStrategy`]) pairs a redaction method with its configuration
//! parameters. [`Strategy`] unifies them under a single tagged enum for
//! policy rules and pipeline decisions.

mod audio;
mod image;
mod text;

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::AudioStrategy;
pub use self::image::ImageStrategy;
pub use self::text::TextStrategy;

/// Unified redaction strategy across all modalities.
///
/// Wraps a per-modality strategy variant carrying the method and its
/// configuration parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(From, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Strategy {
    /// Text/tabular redaction strategy.
    Text(TextStrategy),
    /// Image redaction strategy.
    Image(ImageStrategy),
    /// Audio redaction strategy.
    Audio(AudioStrategy),
}

impl Strategy {
    /// Whether the redaction is reversible (the original value can be
    /// recovered from the redacted output).
    ///
    /// Only [`TextStrategy::Encrypt`] and [`TextStrategy::Tokenize`]
    /// are reversible: Encrypt uses key-based decryption, Tokenize
    /// uses vault-based detokenization. All other strategies are
    /// destructive.
    pub fn is_reversible(&self) -> bool {
        matches!(
            self,
            Self::Text(TextStrategy::Encrypt { .. } | TextStrategy::Tokenize { .. })
        )
    }
}
