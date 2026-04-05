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
    /// For Text/Tabular locations this is the detected text itself; for
    /// Image/Audio it is the text extracted via OCR or STT. The value
    /// is populated during detection/extraction and is **not serialized**
    /// in API responses to prevent data leaks. Use [`Document::value_at`]
    /// to extract from the source document instead.
    pub fn text_value(&self) -> Option<&str> {
        match self {
            Self::Text(loc) if !loc.text.is_empty() => Some(&loc.text),
            Self::Image(loc) => loc.extracted_text.as_deref(),
            Self::Audio(loc) => loc.extracted_text.as_deref(),
            Self::Tabular(loc) if !loc.text.is_empty() => Some(&loc.text),
            _ => None,
        }
    }
}

/// Diagnostic display format for logging; not intended for round-trip
/// parsing.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{BoundingBox, TimeSpan};

    fn text(start: usize, end: usize) -> Location {
        Location::Text(
            TextLocation::builder()
                .with_start_offset(start)
                .with_end_offset(end)
                .build()
                .unwrap(),
        )
    }

    fn text_with_value(val: &str, start: usize, end: usize) -> Location {
        Location::Text(
            TextLocation::builder()
                .with_text(val)
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

    // -- as_* accessors --

    #[test]
    fn as_text_correct_variant() {
        let loc = text(0, 5);
        assert!(loc.as_text().is_some());
        assert!(loc.as_image().is_none());
    }

    #[test]
    fn as_image_correct_variant() {
        let loc = image();
        assert!(loc.as_image().is_some());
        assert!(loc.as_text().is_none());
    }

    #[test]
    fn as_audio_correct_variant() {
        let loc = audio();
        assert!(loc.as_audio().is_some());
        assert!(loc.as_tabular().is_none());
    }

    #[test]
    fn as_tabular_correct_variant() {
        let loc = tabular(0, 0);
        assert!(loc.as_tabular().is_some());
        assert!(loc.as_audio().is_none());
    }

    // -- text_value --

    #[test]
    fn text_value_with_text() {
        assert_eq!(text_with_value("hello", 0, 5).text_value(), Some("hello"));
    }

    #[test]
    fn text_value_empty_is_none() {
        assert_eq!(text(0, 5).text_value(), None);
    }

    #[test]
    fn text_value_image_extracted() {
        let loc = Location::Image(ImageLocation {
            bounding_box: BoundingBox::default(),
            extracted_text: Some("ocr result".into()),
            image_id: None,
            page_number: None,
        });
        assert_eq!(loc.text_value(), Some("ocr result"));
    }

    #[test]
    fn text_value_image_none() {
        assert_eq!(image().text_value(), None);
    }

    #[test]
    fn text_value_tabular() {
        let loc = Location::Tabular(
            TabularLocation::builder()
                .with_text("cell")
                .with_row_index(0usize)
                .with_column_index(0usize)
                .build()
                .unwrap(),
        );
        assert_eq!(loc.text_value(), Some("cell"));
    }

    // -- Display --

    #[test]
    fn display_text() {
        assert_eq!(text(0, 10).to_string(), "text:0..10");
    }

    #[test]
    fn display_tabular() {
        assert_eq!(tabular(2, 3).to_string(), "tabular:r2c3");
    }

    // -- cross-modality overlap --

    #[test]
    fn cross_modality_no_overlap() {
        assert!(!text(0, 10).overlaps(&image()));
        assert!(!image().overlaps(&audio()));
        assert!(!audio().overlaps(&tabular(0, 0)));
    }
}
