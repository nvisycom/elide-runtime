//! Span size comparison for deduplication.
//!
//! Compares the spatial/temporal extent of two same-modality locations
//! to select the most representative span when merging entities.

use std::cmp::Ordering;

use nvisy_ontology::modality::{Audio, Image, Tabular, Text};

/// Extension trait for comparing the spatial/temporal extent of two
/// locations of the same modality.
///
/// Used during deduplication to select the most representative span
/// when merging a group of duplicate entities (e.g. prefer "John Smith"
/// over "John", or the larger bounding box).
///
/// Size metric per modality:
/// - **Text**: byte length (`end - start`).
/// - **Image**: bounding box area (`width * height`).
/// - **Audio**: time span duration.
/// - **Tabular**: cell text length.
pub trait SpanSize {
    fn span_cmp(&self, other: &Self) -> Option<Ordering>;
}

impl SpanSize for Text {
    fn span_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.len().cmp(&other.len()))
    }
}

impl SpanSize for Image {
    fn span_cmp(&self, other: &Self) -> Option<Ordering> {
        self.area().partial_cmp(&other.area())
    }
}

impl SpanSize for Audio {
    fn span_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.time_span
                .duration_us()
                .cmp(&other.time_span.duration_us()),
        )
    }
}

impl SpanSize for Tabular {
    fn span_cmp(&self, other: &Self) -> Option<Ordering> {
        let len_a = self
            .end_offset
            .unwrap_or(0)
            .saturating_sub(self.start_offset.unwrap_or(0));
        let len_b = other
            .end_offset
            .unwrap_or(0)
            .saturating_sub(other.start_offset.unwrap_or(0));
        Some(len_a.cmp(&len_b))
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use nvisy_ontology::primitive::{BoundingBox, TimeSpan};

    use super::*;

    #[test]
    fn text_larger_span_wins() {
        let a = Text::new(0, 10);
        let b = Text::new(0, 5);
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.span_cmp(&a), Some(Ordering::Less));
    }

    #[test]
    fn text_equal_spans() {
        let a = Text::new(0, 5);
        let b = Text::new(3, 8);
        assert_eq!(a.span_cmp(&b), Some(Ordering::Equal));
    }

    #[test]
    fn image_larger_area_wins() {
        let a = Image::new(BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        });
        let b = Image::new(BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        });
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
        assert_eq!(b.span_cmp(&a), Some(Ordering::Less));
    }

    #[test]
    fn audio_longer_duration_wins() {
        let a = Audio::new(TimeSpan::new(0, 10_000_000));
        let b = Audio::new(TimeSpan::new(0, 5_000_000));
        assert_eq!(a.span_cmp(&b), Some(Ordering::Greater));
    }
}
