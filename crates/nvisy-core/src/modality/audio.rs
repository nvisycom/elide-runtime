//! Audio modality coordinate type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Modality, Overlap};
use crate::primitive::TimeSpan;

/// A time interval within audio content.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    /// Time interval.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this interval to a specific audio document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

impl Audio {
    /// Create an [`Audio`] covering `time_span`, with no speaker
    /// attribution or audio-document link.
    pub fn new(time_span: TimeSpan) -> Self {
        Self {
            time_span,
            speaker_id: None,
            audio_id: None,
        }
    }
}

impl Modality for Audio {}

impl Overlap for Audio {
    /// Two audio intervals overlap only when they target the same
    /// stream (matching `audio_id`) on the same `speaker_id` and
    /// their time spans intersect. Two voices captured on the same
    /// time interval are physically distinct redaction targets
    /// (source separation can suppress one speaker while keeping
    /// another), so different speaker IDs are not treated as
    /// overlapping even when the time spans intersect.
    fn overlaps(&self, other: &Self) -> bool {
        self.audio_id == other.audio_id
            && self.speaker_id == other.speaker_id
            && self.time_span.overlaps(&other.time_span)
    }
}
