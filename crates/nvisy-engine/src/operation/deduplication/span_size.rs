//! Span size comparison for deduplication.
//!
//! Compares the spatial/temporal extent of two same-modality locations
//! to select the most representative span when merging entities.

use std::cmp::Ordering;

use nvisy_ontology::entity::Location;

/// Extension trait for comparing the spatial/temporal extent of two
/// same-modality [`Location`]s.
///
/// Used during deduplication to select the most representative span
/// when merging a group of duplicate entities (e.g. prefer "John Smith"
/// over "John", or the larger bounding box).
///
/// Returns `None` for cross-modality comparisons (meaningless).
/// Returns `Some(cmp)` with a standard [`Ordering`](std::cmp::Ordering)
/// for same-modality pairs.
///
/// Size metric per modality:
/// - **Text**: byte length (`end_offset - start_offset`).
/// - **Image**: bounding box area (`width * height`).
/// - **Audio**: time span duration.
/// - **Tabular**: cell text length.
pub(crate) trait SpanSize {
    /// Compare the extent of two locations.
    ///
    /// Returns `None` if the locations are different modalities.
    fn span_cmp(&self, other: &Self) -> Option<Ordering>;
}

impl SpanSize for Location {
    fn span_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => Some(a.len().cmp(&b.len())),
            (Self::Image(a), Self::Image(b)) => a.area().partial_cmp(&b.area()),
            (Self::Audio(a), Self::Audio(b)) => {
                Some(a.time_span.duration_us().cmp(&b.time_span.duration_us()))
            }
            // Tabular: compare intra-cell byte range length. Entities
            // without offsets (whole-cell) compare as equal (both 0).
            (Self::Tabular(a), Self::Tabular(b)) => {
                let len_a = a
                    .end_offset
                    .unwrap_or(0)
                    .saturating_sub(a.start_offset.unwrap_or(0));
                let len_b = b
                    .end_offset
                    .unwrap_or(0)
                    .saturating_sub(b.start_offset.unwrap_or(0));
                Some(len_a.cmp(&len_b))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{AudioLocation, ImageLocation, TabularLocation, TextLocation};
    use nvisy_ontology::math::{BoundingBox, TimeSpan};

    use super::*;

    fn text_loc(start: usize, end: usize) -> Location {
        Location::Text(TextLocation {
            start_offset: start,
            end_offset: end,
            ..Default::default()
        })
    }

    use std::cmp::Ordering;

    #[test]
    fn text_larger_span_wins() {
        let a = text_loc(0, 10);
        let b = text_loc(0, 5);
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.span_cmp(&a), Some(Ordering::Less));
    }

    #[test]
    fn text_equal_spans() {
        let a = text_loc(0, 5);
        let b = text_loc(3, 8);
        assert_eq!(a.span_cmp(&b), Some(Ordering::Equal));
    }

    #[test]
    fn image_larger_area_wins() {
        let a = Location::Image(
            ImageLocation::builder()
                .with_bounding_box(BoundingBox {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                })
                .build()
                .unwrap(),
        );
        let b = Location::Image(
            ImageLocation::builder()
                .with_bounding_box(BoundingBox {
                    x: 0.0,
                    y: 0.0,
                    width: 50.0,
                    height: 50.0,
                })
                .build()
                .unwrap(),
        );
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.span_cmp(&a), Some(Ordering::Less));
    }

    #[test]
    fn audio_longer_duration_wins() {
        let a = Location::Audio(
            AudioLocation::builder()
                .with_time_span(TimeSpan {
                    start_us: 0,
                    end_us: 10_000_000,
                })
                .build()
                .unwrap(),
        );
        let b = Location::Audio(
            AudioLocation::builder()
                .with_time_span(TimeSpan {
                    start_us: 0,
                    end_us: 5_000_000,
                })
                .build()
                .unwrap(),
        );
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
    }

    #[test]
    fn cross_modality_returns_none() {
        let text = text_loc(0, 10);
        let image = Location::Image(
            ImageLocation::builder()
                .with_bounding_box(BoundingBox {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                })
                .build()
                .unwrap(),
        );
        assert_eq!(text.span_cmp(&image), None);
    }
}
