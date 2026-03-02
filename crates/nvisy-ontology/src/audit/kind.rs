//! Tagged enum discriminating audit entry categories.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{InferenceAction, LifecycleAction, ProcessingAction};

/// Classifies the activity recorded by a [`FileAuditEntry`](super::FileAuditEntry).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FileAuditEntryKind {
    // Inference
    /// OCR text extraction.
    Ocr(InferenceAction),
    /// Audio/video transcription.
    Transcription(InferenceAction),
    /// Named-entity recognition.
    Ner(InferenceAction),
    /// Face or object detection.
    ComputerVision(InferenceAction),
    /// Content or context translation.
    Translation(InferenceAction),
    /// Content classification.
    Classification(InferenceAction),
    /// Content summarization.
    Summarization(InferenceAction),

    // Processing
    /// Regex or dictionary matching.
    PatternMatch(ProcessingAction),
    /// Policy rule evaluation.
    PolicyEvaluation(ProcessingAction),
    /// Redaction application.
    Redaction(ProcessingAction),

    // Lifecycle
    /// File ingest or load.
    Ingest(LifecycleAction),
    /// File publish or deliver.
    Publish(LifecycleAction),
}
