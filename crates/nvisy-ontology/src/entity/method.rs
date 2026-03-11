//! Detection method classification.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Method used to detect a sensitive entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DetectionMethod {
    /// Regular expression pattern matching.
    Regex,
    /// Lookup in a known-value dictionary.
    Dictionary,
    /// Named-entity recognition via AI model.
    Ner,
    /// Contextual NLP analysis (discourse-level understanding).
    ContextualNlp,
    /// OCR text extraction with bounding boxes.
    Ocr,
    /// Face detection in images.
    FaceDetection,
    /// Object detection in images.
    ObjectDetection,
    /// Entity detection from speech transcription.
    SpeechTranscript,
    /// Speaker-identified audio segment for redaction.
    SpeakerRedaction,
    /// Multiple methods combined to produce a single detection.
    Composite,
    /// User-provided annotations.
    Manual,
}
