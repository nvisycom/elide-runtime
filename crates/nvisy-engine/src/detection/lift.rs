//! [`LiftFromBlock`]: per-modality lift from block-text-byte
//! range to absolute `M` location, using the block's source-
//! mapping spans.
//!
//! Recognizers operate on a block's flat text and emit entities
//! whose offsets are local to that block. The detection driver
//! lifts each entity's `[start, end)` range to an absolute `M`
//! location by walking the block's [`Span<M>`]s — each span maps a
//! sub-range of the block text to its originating modality
//! coordinate, and overlapping spans contribute their `source: M`
//! to the union that becomes the entity's final location.
//!
//! [`Span<M>`]: nvisy_ontology::document::Span

use nvisy_ontology::document::Span;
use nvisy_ontology::modality::{Audio, Image, Modality, Tabular, Text};

/// Map a block-text byte range to an absolute `M` location using
/// the block's spans.
///
/// Returns `None` when no span overlaps the requested range — the
/// detection driver discards such entities since there's no way to
/// place them in modality coordinates.
pub trait LiftFromBlock: Modality + Sized {
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self>;
}

impl LiftFromBlock for Text {
    /// For text the block normally has exactly one span covering
    /// `0..text.len()` whose `source: Text` carries the
    /// document-relative offsets. The entity's lifted location
    /// shifts `[start, end)` into that span's source range.
    /// Multi-span text blocks (rare today) take the first
    /// overlapping span as the anchor.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let span = spans.iter().find(|s| s.overlaps(start, end))?;
        let span_text_start = span.text_start;
        let source_base = span.source.start_offset;
        let lifted_start = source_base + start.saturating_sub(span_text_start);
        let lifted_end = source_base + end.saturating_sub(span_text_start);
        Some(Text::new(lifted_start, lifted_end))
    }
}

impl LiftFromBlock for Tabular {
    /// Tabular cells live in distinct `(row, col)` coordinates. When
    /// the entity spans a single cell, the cell's coordinates are
    /// returned with intra-cell `start_offset`/`end_offset` adjusted
    /// to the in-cell substring. Cross-cell matches return the
    /// first overlapping cell's coordinates — they're rare and
    /// downstream redaction operates per-cell anyway.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let span = spans.iter().find(|s| s.overlaps(start, end))?;
        let local_start = start.saturating_sub(span.text_start);
        let local_end = end.saturating_sub(span.text_start);
        let cell = &span.source;
        Some(Tabular {
            row_index: cell.row_index,
            column_index: cell.column_index,
            start_offset: Some(local_start),
            end_offset: Some(local_end),
            column_name: cell.column_name.clone(),
            sheet_name: cell.sheet_name.clone(),
        })
    }
}

impl LiftFromBlock for Image {
    /// Image entity location is the union of overlapping word-span
    /// bounding boxes. Per-modality `Mergeable` handles polygon
    /// drop semantics; we fold via [`Image::bounding_box`] unions
    /// directly.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let mut iter = spans.iter().filter(|s| s.overlaps(start, end));
        let first = iter.next()?;
        let mut bbox = first.source.bounding_box;
        let image_id = first.source.image_id;
        let page_number = first.source.page_number;
        for s in iter {
            bbox = bbox.union(&s.source.bounding_box);
        }
        Some(Image {
            bounding_box: bbox,
            polygon: None,
            image_id,
            page_number,
        })
    }
}

impl LiftFromBlock for Audio {
    /// Audio entity location is the union of overlapping word-span
    /// time intervals on the same speaker.
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self> {
        let mut iter = spans.iter().filter(|s| s.overlaps(start, end));
        let first = iter.next()?;
        let mut time_span = first.source.time_span;
        let speaker_id = first.source.speaker_id.clone();
        let audio_id = first.source.audio_id;
        for s in iter {
            time_span = time_span.union(&s.source.time_span);
        }
        Some(Audio {
            time_span,
            speaker_id,
            audio_id,
        })
    }
}
