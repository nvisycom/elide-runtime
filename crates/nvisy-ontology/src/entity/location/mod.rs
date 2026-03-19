//! Modality-specific entity location types.

mod audio;
mod image;
mod tabular;
mod text;

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::AudioLocation;
pub use self::image::ImageLocation;
pub use self::tabular::TabularLocation;
pub use self::text::TextLocation;

/// Trait for checking whether two values overlap spatially or temporally.
pub trait Overlap {
    fn overlaps(&self, other: &Self) -> bool;
}

impl<T: Overlap> Overlap for Option<T> {
    /// Two `None`s are considered distinct (no overlap).
    fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.overlaps(b),
            _ => false,
        }
    }
}

/// A modality-specific location for a detected entity.
///
/// Exactly one variant is set per entity, enforcing the invariant that
/// an entity exists in a single modality.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize, JsonSchema)]
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

impl Overlap for Location {
    fn overlaps(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a.overlaps(b),
            (Self::Image(a), Self::Image(b)) => a.overlaps(b),
            (Self::Audio(a), Self::Audio(b)) => a.overlaps(b),
            (Self::Tabular(a), Self::Tabular(b)) => a.overlaps(b),
            _ => false,
        }
    }
}
