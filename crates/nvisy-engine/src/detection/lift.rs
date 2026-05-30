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
//! The reverse direction — taking a modality target (e.g. an
//! annotation's region) and projecting it onto block-text byte
//! offsets — is [`ProjectIntoBlock`]. Used when the engine needs
//! to surface user-supplied annotation regions to a recognizer
//! that only speaks block-local text offsets (LLM hint
//! adjudication, today).
//!
//! [`Span<M>`]: nvisy_ontology::document::Span

use nvisy_ontology::document::Span;
use nvisy_ontology::modality::{Audio, Image, Modality, Overlap, Tabular, Text};

/// Map a block-text byte range to an absolute `M` location using
/// the block's spans.
///
/// Returns `None` when no span overlaps the requested range — the
/// detection driver discards such entities since there's no way to
/// place them in modality coordinates.
pub trait LiftFromBlock: Modality + Sized {
    fn lift_from_block(spans: &[Span<Self>], start: usize, end: usize) -> Option<Self>;
}

/// Project a modality-typed `target` (e.g. an annotation region)
/// onto block-text byte offsets within the given block's spans.
///
/// Returns the `[block_text_start, block_text_end)` range of block
/// text whose source spans overlap `target`, or `None` when no
/// span overlaps. The returned range is the union of
/// `span.text_start..span.text_end` over all overlapping spans,
/// suitable for handing to a recognizer that consumes block-local
/// text offsets.
///
/// The translation fidelity is per-modality:
///
/// - [`Text`]: spans typically cover the full block text (one span
///   per block, `text_start=0..text_end=text.len()`), and the
///   span's `source: Text` carries doc-absolute byte offsets.
///   Projection narrows the block-text range to just the
///   `target`-overlapping portion (sub-span precision).
/// - [`Tabular`], [`Image`], [`Audio`]: spans typically cover a
///   whole cell / word / phoneme. Projection returns the union of
///   those spans' block-text ranges as-is (span-granular
///   precision); we can't sub-divide an image word into pixels.
pub trait ProjectIntoBlock: Modality + Sized {
    fn project_into_block(spans: &[Span<Self>], target: &Self) -> Option<(usize, usize)>;
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
        let source_base = span.source.start;
        let lifted_start = source_base + start.saturating_sub(span_text_start);
        let lifted_end = source_base + end.saturating_sub(span_text_start);
        Some(Text::new(lifted_start, lifted_end))
    }
}

impl ProjectIntoBlock for Text {
    /// Inverse of [`lift_from_block`]: shift `target`'s doc-absolute
    /// byte range into block-text coordinates within the first
    /// overlapping span. Sub-span precision — clamps to the span's
    /// overlap with `target`.
    ///
    /// [`lift_from_block`]: LiftFromBlock::lift_from_block
    fn project_into_block(spans: &[Span<Self>], target: &Self) -> Option<(usize, usize)> {
        let span = spans.iter().find(|s| s.source.overlaps(target))?;
        let span_text_start = span.text_start;
        let source_base = span.source.start;
        let source_end = span.source.end;
        // Clamp target to the span's source range first.
        let clamped_start = target.start.max(source_base);
        let clamped_end = target.end.min(source_end);
        if clamped_start >= clamped_end {
            return None;
        }
        let block_start = span_text_start + (clamped_start - source_base);
        let block_end = span_text_start + (clamped_end - source_base);
        Some((block_start, block_end))
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
    /// bounding boxes, folded via [`BoundingBox::union`].
    ///
    /// [`BoundingBox::union`]: nvisy_ontology::primitive::BoundingBox::union
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

/// Span-granular projection used by [`Tabular`], [`Image`], and
/// [`Audio`]: a modality target's overlap with the block's spans
/// is reported as the union of those spans' block-text ranges.
/// We don't sub-divide an image word into pixels or an audio word
/// into samples — the recognizer sees whole-span text either way.
fn union_block_range<M: Modality + Overlap>(
    spans: &[Span<M>],
    target: &M,
) -> Option<(usize, usize)> {
    let mut iter = spans.iter().filter(|s| s.source.overlaps(target));
    let first = iter.next()?;
    let mut lo = first.text_start;
    let mut hi = first.text_end;
    for s in iter {
        if s.text_start < lo {
            lo = s.text_start;
        }
        if s.text_end > hi {
            hi = s.text_end;
        }
    }
    Some((lo, hi))
}

impl ProjectIntoBlock for Tabular {
    fn project_into_block(spans: &[Span<Self>], target: &Self) -> Option<(usize, usize)> {
        union_block_range(spans, target)
    }
}

impl ProjectIntoBlock for Image {
    fn project_into_block(spans: &[Span<Self>], target: &Self) -> Option<(usize, usize)> {
        union_block_range(spans, target)
    }
}

impl ProjectIntoBlock for Audio {
    fn project_into_block(spans: &[Span<Self>], target: &Self) -> Option<(usize, usize)> {
        union_block_range(spans, target)
    }
}
