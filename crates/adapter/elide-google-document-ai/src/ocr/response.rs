//! Translation from a Document AI `Document` into elide's image
//! vocabulary.
//!
//! Document AI returns text once, on the document, and every layout
//! element points into it by byte offset. Geometry arrives twice — as
//! pixel `vertices` and as `normalized_vertices` — and this reads the
//! pixel form, which is what [`ImageLocation`] holds.

use elide_core::primitive::Confidence;
use elide_image::modality::{ImageLocation, LayoutRegion};
use elide_image::ocr::OcrResponse;
use elide_image::primitive::{BoundingBox, Point};
use google_cloud_documentai_v1::model::document::page::Layout;
use google_cloud_documentai_v1::model::{BoundingPoly, Document};

/// The transcript of a processed document.
///
/// Emits one region per **token** — Document AI's word level, and the
/// finest granularity it reports. Blocks, paragraphs and lines are
/// discarded: elide's [`Layout`] is a flat list, and the token level is
/// where a redaction pipeline wants its spans.
///
/// [`Layout`]: elide_image::modality::Layout
pub(super) fn decode(document: &Document) -> OcrResponse {
    let mut regions = Vec::new();
    for (index, page) in document.pages.iter().enumerate() {
        // `page_number` is 1-based on the wire, but a processor may leave
        // it at zero; the enumeration index is the reliable fallback.
        let page_number = u32::try_from(page.page_number)
            .ok()
            .filter(|n| *n > 0)
            .or_else(|| u32::try_from(index + 1).ok());

        for token in &page.tokens {
            if let Some(region) = token
                .layout
                .as_ref()
                .and_then(|layout| self::region(layout, &document.text, page_number))
            {
                regions.push(region);
            }
        }
    }
    OcrResponse::new(regions)
}

/// One region from a layout element, or `None` when it has no usable
/// geometry or resolves to no text.
fn region(layout: &Layout, text: &str, page_number: Option<u32>) -> Option<LayoutRegion> {
    let bounding_box = layout.bounding_poly.as_ref().and_then(self::bounding_box)?;
    let content = self::text(layout, text)?;

    let location = ImageLocation {
        bounding_box,
        polygon: None,
        page: page_number,
    };
    // Confidence is already `0.0..=1.0` on the wire; `clamped` guards a
    // provider that ever reports outside it rather than rescaling.
    Some(
        LayoutRegion::new(location, content)
            .with_confidence(Confidence::clamped(layout.confidence)),
    )
}

/// The axis-aligned extent of a polygon's pixel vertices.
///
/// Reads `vertices`, not `normalized_vertices`: [`ImageLocation`] holds
/// pixels, and the response carries both, so no page dimensions are
/// needed to convert. A polygon with no pixel vertices yields `None`
/// rather than a box at the origin — a region placed at (0,0) would
/// redact the wrong part of the image.
fn bounding_box(poly: &BoundingPoly) -> Option<BoundingBox<f64>> {
    let xs: Vec<f64> = poly.vertices.iter().map(|v| f64::from(v.x)).collect();
    let ys: Vec<f64> = poly.vertices.iter().map(|v| f64::from(v.y)).collect();
    if xs.is_empty() {
        return None;
    }

    let (min_x, max_x) = extent(&xs)?;
    let (min_y, max_y) = extent(&ys)?;
    Some(BoundingBox::new(
        Point::new(min_x, min_y),
        Point::new(max_x, max_y),
    ))
}

/// The smallest and largest of `values`, or `None` when any is not finite.
///
/// `f64` has no total order, so this avoids `partial_cmp().unwrap()`: a
/// NaN coordinate would panic there.
fn extent(values: &[f64]) -> Option<(f64, f64)> {
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some((min, max))
}

/// The text a layout's anchors point at, trimmed.
///
/// Document AI carries text once on the document and addresses it by byte
/// offset. An anchor outside the text, or one whose end precedes its
/// start, is skipped rather than clamped: a mis-sliced span in a redaction
/// pipeline redacts the wrong bytes. Multi-byte boundaries are honoured,
/// so a slice landing mid-character is dropped rather than panicking.
fn text(layout: &Layout, document_text: &str) -> Option<String> {
    let anchor = layout.text_anchor.as_ref()?;
    let mut content = String::new();
    for segment in &anchor.text_segments {
        let start = usize::try_from(segment.start_index).ok()?;
        let end = usize::try_from(segment.end_index).ok()?;
        if end <= start || end > document_text.len() {
            continue;
        }
        if !document_text.is_char_boundary(start) || !document_text.is_char_boundary(end) {
            continue;
        }
        content.push_str(&document_text[start..end]);
    }

    let trimmed = content.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use google_cloud_documentai_v1::model::Vertex;
    use google_cloud_documentai_v1::model::document::TextAnchor;
    use google_cloud_documentai_v1::model::document::text_anchor::TextSegment;

    use super::*;

    fn poly(points: &[(i32, i32)]) -> BoundingPoly {
        BoundingPoly::new().set_vertices(
            points
                .iter()
                .map(|(x, y)| Vertex::new().set_x(*x).set_y(*y))
                .collect::<Vec<_>>(),
        )
    }

    fn layout(points: &[(i32, i32)], start: i64, end: i64) -> Layout {
        Layout::new()
            .set_confidence(0.9)
            .set_bounding_poly(poly(points))
            .set_text_anchor(TextAnchor::new().set_text_segments(vec![
                TextSegment::new().set_start_index(start).set_end_index(end),
            ]))
    }

    /// Pixel vertices become an axis-aligned box, and the anchor's byte
    /// range slices the document text.
    #[test]
    fn maps_pixel_vertices_and_anchored_text() {
        let text = "hello world";
        let decoded = region(
            &layout(&[(10, 20), (60, 20), (60, 40), (10, 40)], 0, 5),
            text,
            Some(1),
        )
        .expect("a usable region");

        assert_eq!(decoded.text, "hello");
        assert_eq!(decoded.region.bounding_box.min, Point::new(10.0, 20.0));
        assert_eq!(decoded.region.bounding_box.max, Point::new(60.0, 40.0));
        assert_eq!(decoded.region.page, Some(1));
        assert!(decoded.confidence.is_some());
    }

    /// An anchor running past the text is skipped rather than clamped: a
    /// mis-sliced span redacts the wrong bytes.
    #[test]
    fn drops_anchors_past_the_end_of_the_text() {
        assert!(region(&layout(&[(0, 0), (10, 10)], 0, 99), "short", None).is_none());
    }

    /// An inverted or empty anchor yields no text, so no region.
    #[test]
    fn drops_inverted_and_empty_anchors() {
        assert!(region(&layout(&[(0, 0), (10, 10)], 5, 2), "hello world", None).is_none());
        assert!(region(&layout(&[(0, 0), (10, 10)], 3, 3), "hello world", None).is_none());
    }

    /// A slice landing mid-character is dropped rather than panicking.
    ///
    /// Document AI indexes bytes, and "é" is two of them, so an offset of
    /// 1 is not a character boundary.
    #[test]
    fn drops_slices_inside_a_multibyte_character() {
        assert!(region(&layout(&[(0, 0), (10, 10)], 0, 1), "éclair", None).is_none());
        // The whole character is fine.
        let decoded = region(&layout(&[(0, 0), (10, 10)], 0, 2), "éclair", None).unwrap();
        assert_eq!(decoded.text, "é");
    }

    /// A layout with no pixel vertices yields nothing rather than a box at
    /// the origin, which would redact the wrong part of the image.
    #[test]
    fn drops_layouts_without_pixel_vertices() {
        let bare = Layout::new().set_confidence(0.9).set_text_anchor(
            TextAnchor::new()
                .set_text_segments(vec![TextSegment::new().set_start_index(0).set_end_index(5)]),
        );
        assert!(region(&bare, "hello", None).is_none());
    }

    /// `f64` has no total order, so a non-finite coordinate is rejected
    /// rather than reached via `partial_cmp().unwrap()`.
    #[test]
    fn extent_rejects_non_finite_values() {
        assert_eq!(extent(&[1.0, 5.0, 3.0]), Some((1.0, 5.0)));
        assert!(extent(&[1.0, f64::NAN]).is_none());
    }
}
