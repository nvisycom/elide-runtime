//! [`Transcription`]: the value emitted by an [`SttExtractor`].
//!
//! [`SttExtractor`]: super::SttExtractor

use crate::backend::TranscribedSegment;

/// Output of an STT extraction pass: an ordered list of
/// [`TranscribedSegment`]s covering the source audio.
///
/// Empty if the backend returned nothing (e.g. silence or the no-op
/// backend).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Transcription {
    /// Segments in source order.
    pub segments: Vec<TranscribedSegment>,
}

impl Transcription {
    /// Build a transcription from segments.
    #[must_use]
    pub fn new(segments: Vec<TranscribedSegment>) -> Self {
        Self { segments }
    }

    /// Concatenated text of every segment, joined with a single space.
    ///
    /// Convenience for downstream callers (NER recognizers) that want
    /// the recognised text as one string. Segment-level timestamps are
    /// preserved in [`segments`] for callers that need them.
    ///
    /// [`segments`]: Self::segments
    pub fn full_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
