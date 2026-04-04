//! Modality-specific entity location types.

mod audio;
mod image;
mod tabular;
mod text;

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::{AudioLocation, AudioLocationBuilder};
pub use self::image::{ImageLocation, ImageLocationBuilder};
pub use self::tabular::{TabularLocation, TabularLocationBuilder};
pub use self::text::{TextLocation, TextLocationBuilder};

/// Trait for checking whether two values overlap spatially or temporally.
pub trait Overlap {
    fn overlaps(&self, other: &Self) -> bool;
}

/// A modality-specific location for a detected entity.
///
/// Exactly one variant is set per entity, enforcing the invariant that
/// an entity exists in a single modality.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
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

impl Location {
    /// The text value at this location, if available.
    ///
    /// Returns the internally stored value (not serialized in API
    /// responses). Use [`Document::value_at`] to extract from the
    /// source document instead.
    pub fn text_value(&self) -> Option<&str> {
        match self {
            Self::Text(loc) if !loc.value.is_empty() => Some(&loc.value),
            Self::Image(loc) => loc.value.as_deref(),
            Self::Audio(loc) => loc.value.as_deref(),
            Self::Tabular(loc) if !loc.value.is_empty() => Some(&loc.value),
            _ => None,
        }
    }

    /// Compare the span size of two same-modality locations.
    ///
    /// Returns `None` if the locations are different modalities (cross-
    /// modality size comparison is meaningless). Returns `Some(true)`
    /// if `self` is at least as large as `other`.
    ///
    /// Size metric per modality:
    /// - **Text**: byte length of the span (`end - start`).
    /// - **Image**: bounding box area in pixels.
    /// - **Audio**: time span duration in seconds.
    /// - **Tabular**: value (cell content) length.
    pub fn is_at_least_as_large(&self, other: &Self) -> Option<bool> {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => {
                let sa = a.end_offset.saturating_sub(a.start_offset);
                let sb = b.end_offset.saturating_sub(b.start_offset);
                Some(sa >= sb)
            }
            (Self::Image(a), Self::Image(b)) => {
                let area_a = a.bounding_box.width * a.bounding_box.height;
                let area_b = b.bounding_box.width * b.bounding_box.height;
                Some(area_a >= area_b)
            }
            (Self::Audio(a), Self::Audio(b)) => {
                Some(a.time_span.duration_secs() >= b.time_span.duration_secs())
            }
            (Self::Tabular(a), Self::Tabular(b)) => Some(a.value.len() >= b.value.len()),
            _ => None,
        }
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(loc) => write!(f, "text:{}..{}", loc.start_offset, loc.end_offset),
            Self::Image(loc) => {
                let bb = &loc.bounding_box;
                write!(
                    f,
                    "image:{:.0},{:.0} {:.0}x{:.0}",
                    bb.x, bb.y, bb.width, bb.height
                )
            }
            Self::Audio(loc) => {
                let ts = &loc.time_span;
                write!(f, "audio:{:.2}s..{:.2}s", ts.start_secs(), ts.end_secs())
            }
            Self::Tabular(loc) => {
                write!(f, "tabular:r{}c{}", loc.row_index, loc.column_index)
            }
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
