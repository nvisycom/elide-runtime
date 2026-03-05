//! Modality-specific entity location types.

mod audio;
mod image;
mod tabular;
mod text;

pub use audio::AudioLocation;
pub use image::ImageLocation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use tabular::TabularLocation;
pub use text::TextLocation;

/// A modality-specific location for a detected entity.
///
/// Exactly one variant is set per entity, enforcing the invariant that
/// an entity exists in a single modality.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Location {
    /// Entity found in text content.
    Text(TextLocation),
    /// Entity found in an image.
    Image(ImageLocation),
    /// Entity found in tabular data.
    Tabular(TabularLocation),
    /// Entity found in audio.
    Audio(AudioLocation),
}

impl Location {
    /// If this is a text location, return a reference to it.
    pub fn as_text(&self) -> Option<&TextLocation> {
        match self {
            Self::Text(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is an image location, return a reference to it.
    pub fn as_image(&self) -> Option<&ImageLocation> {
        match self {
            Self::Image(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is a tabular location, return a reference to it.
    pub fn as_tabular(&self) -> Option<&TabularLocation> {
        match self {
            Self::Tabular(loc) => Some(loc),
            _ => None,
        }
    }

    /// If this is an audio location, return a reference to it.
    pub fn as_audio(&self) -> Option<&AudioLocation> {
        match self {
            Self::Audio(loc) => Some(loc),
            _ => None,
        }
    }
}

impl From<TextLocation> for Location {
    fn from(loc: TextLocation) -> Self {
        Self::Text(loc)
    }
}

impl From<ImageLocation> for Location {
    fn from(loc: ImageLocation) -> Self {
        Self::Image(loc)
    }
}

impl From<TabularLocation> for Location {
    fn from(loc: TabularLocation) -> Self {
        Self::Tabular(loc)
    }
}

impl From<AudioLocation> for Location {
    fn from(loc: AudioLocation) -> Self {
        Self::Audio(loc)
    }
}
