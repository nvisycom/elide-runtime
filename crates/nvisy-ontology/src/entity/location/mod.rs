//! Modality-specific entity location types.

mod audio;
mod image;
mod tabular;
mod text;

use std::fmt;

use derive_more::From;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::audio::{AudioLocation, AudioLocationBuilder};
pub use self::image::{ImageLocation, ImageLocationBuilder};
pub use self::tabular::{TabularLocation, TabularLocationBuilder};
pub use self::text::{TextLocation, TextLocationBuilder};

/// Trait for checking whether two locations overlap.
///
/// The semantics of "overlap" vary by modality:
/// - **Text**: byte-range interval overlap (`start < other.end && other.start < end`).
/// - **Image**: bounding box intersection.
/// - **Audio**: time span overlap.
/// - **Tabular**: same cell (row + column), with optional intra-cell
///   byte-range check when offsets are present.
///
/// Cross-modality comparisons on [`Location`] always return `false`.
pub trait Overlap {
    fn overlaps(&self, other: &Self) -> bool;
}

/// Trait for combining two values into one when they can be reconciled.
///
/// Used by [`Redactions`] (and any other collection that groups
/// targets) under a merge policy: when two entries collide (per
/// [`Overlap`]), the collection asks both the location and the
/// payload whether they can fuse. Returns `Some(merged)` when the
/// two can be combined (e.g. unioned bounding boxes, identical
/// outputs), `None` when they cannot (e.g. different tabular cells,
/// conflicting replacement strings).
///
/// [`Redactions`]: https://docs.rs/nvisy-codec/latest/nvisy_codec/transform/struct.Redactions.html
pub trait Mergeable: Sized {
    fn try_merge(self, other: Self) -> Option<Self>;
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

/// Diagnostic display format for logging; not intended for round-trip
/// parsing.
impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::{BoundingBox, TimeSpan};

    fn text(start: usize, end: usize) -> Location {
        Location::Text(
            TextLocation::builder()
                .with_start_offset(start)
                .with_end_offset(end)
                .build()
                .unwrap(),
        )
    }

    fn image() -> Location {
        Location::Image(
            ImageLocation::builder()
                .with_bounding_box(BoundingBox {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                })
                .build()
                .unwrap(),
        )
    }

    fn audio() -> Location {
        Location::Audio(
            AudioLocation::builder()
                .with_time_span(TimeSpan {
                    start_us: 0,
                    end_us: 1_000_000,
                })
                .build()
                .unwrap(),
        )
    }

    fn tabular(row: usize, col: usize) -> Location {
        Location::Tabular(
            TabularLocation::builder()
                .with_row_index(row)
                .with_column_index(col)
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn cross_modality_no_overlap() {
        assert!(!text(0, 10).overlaps(&image()));
        assert!(!image().overlaps(&audio()));
        assert!(!audio().overlaps(&tabular(0, 0)));
    }
}
