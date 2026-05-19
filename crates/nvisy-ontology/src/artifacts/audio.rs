//! Audio-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitive::{LanguageTag, TimeSpan};

/// A single timestamped segment within a transcription.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    /// The transcribed text for this segment.
    pub text: String,
    /// Time interval of this segment within the audio stream.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Confidence score for this segment in the range `[0.0, 1.0]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// Full transcript produced by speech-to-text extraction.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    /// Timestamped segments composing the full transcript.
    pub segments: Vec<TranscriptSegment>,
    /// BCP-47 language tag of the detected language, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub language: Option<LanguageTag>,
}

impl Transcription {
    /// Returns the full transcript text by joining all segments.
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Returns the total time span covering all segments, or `None` if empty.
    pub fn time_span(&self) -> Option<TimeSpan> {
        let first = self.segments.first()?;
        let last = self.segments.last()?;
        Some(TimeSpan::new(
            first.time_span.start_us,
            last.time_span.end_us,
        ))
    }
}

/// Artifacts produced during processing of audio content.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioArtifacts {
    /// Speech-to-text transcription result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription: Option<Transcription>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_transcription() -> Transcription {
        Transcription {
            segments: vec![
                TranscriptSegment {
                    text: "Hello".to_owned(),
                    time_span: TimeSpan::from_secs(0.0, 1.0),
                    speaker_id: Some("speaker_1".to_owned()),
                    confidence: Some(0.95),
                },
                TranscriptSegment {
                    text: "world".to_owned(),
                    time_span: TimeSpan::from_secs(1.0, 2.0),
                    speaker_id: Some("speaker_2".to_owned()),
                    confidence: Some(0.90),
                },
            ],
            language: Some("en".parse().unwrap()),
        }
    }

    #[test]
    fn text_joins_segments() {
        let t = sample_transcription();
        assert_eq!(t.text(), "Hello world");
    }

    #[test]
    fn text_empty_segments() {
        let t = Transcription {
            segments: vec![],
            language: None,
        };
        assert_eq!(t.text(), "");
    }

    #[test]
    fn time_span_covers_all_segments() {
        let t = sample_transcription();
        let span = t.time_span().unwrap();
        assert_eq!(span.start_us, 0);
        assert_eq!(span.end_us, 2_000_000);
    }

    #[test]
    fn time_span_empty() {
        let t = Transcription {
            segments: vec![],
            language: None,
        };
        assert!(t.time_span().is_none());
    }
}
