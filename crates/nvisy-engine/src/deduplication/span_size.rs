//! Span size comparison for deduplication.
//!
//! Compares the spatial/temporal extent of two same-modality locations
//! to select the most representative span when merging entities.

use std::cmp::Ordering;

use nvisy_ontology::modality::AnyModality;

/// Extension trait for comparing the spatial/temporal extent of two
/// same-modality [`Location`]s.
///
/// Used during deduplication to select the most representative span
/// when merging a group of duplicate entities (e.g. prefer "John Smith"
/// over "John", or the larger bounding box).
///
/// Returns `None` for cross-modality comparisons (meaningless).
/// Returns `Some(cmp)` with a standard [`Ordering`]
/// for same-modality pairs.
///
/// Size metric per modality:
/// - **Text**: byte length (`end_offset - start_offset`).
/// - **Image**: bounding box area (`width * height`).
/// - **Audio**: time span duration.
/// - **Tabular**: cell text length.
///
/// [`Ordering`]: std::cmp::Ordering
pub(super) trait SpanSize {
    /// Compare the extent of two locations.
    ///
    /// Returns `None` if the locations are different modalities.
    fn span_cmp(&self, other: &Self) -> Option<Ordering>;
}

impl SpanSize for AnyModality {
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
    use std::cmp::Ordering;

    use nvisy_ontology::entity::{Audio, Image, Text};
    use nvisy_ontology::primitive::{BoundingBox, TimeSpan};

    use super::*;

    #[test]
    fn text_larger_span_wins() {
        let a = AnyModality::Text(Text::new(0, 10));
        let b = AnyModality::Text(Text::new(0, 5));
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.span_cmp(&a), Some(Ordering::Less));
    }

    #[test]
    fn text_equal_spans() {
        let a = AnyModality::Text(Text::new(0, 5));
        let b = AnyModality::Text(Text::new(3, 8));
        assert_eq!(a.span_cmp(&b), Some(Ordering::Equal));
    }

    #[test]
    fn image_larger_area_wins() {
        let a = AnyModality::Image(Image::new(BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }));
        let b = AnyModality::Image(Image::new(BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        }));
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.span_cmp(&a), Some(Ordering::Less));
    }

    #[test]
    fn audio_longer_duration_wins() {
        let a = AnyModality::Audio(Audio::new(TimeSpan::new(0, 10_000_000)));
        let b = AnyModality::Audio(Audio::new(TimeSpan::new(0, 5_000_000)));
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
    }

    #[test]
    fn cross_modality_returns_none() {
        let text = AnyModality::Text(Text::new(0, 10));
        let image = AnyModality::Image(Image::new(BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }));
        assert_eq!(text.span_cmp(&image), None);
    }
}
