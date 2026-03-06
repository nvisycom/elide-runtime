//! Two-level tagged enum discriminating audit entry categories.

use nvisy_ontology::entity::DetectionMethod;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{InferenceAction, LifecycleAction, ProcessingAction};

/// Inference operation variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InferenceKind {
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
}

impl InferenceKind {
    /// Returns the [`DetectionMethod`] that corresponds to this inference kind.
    pub fn detection_method(&self) -> DetectionMethod {
        match self {
            Self::Ocr(_) => DetectionMethod::Ocr,
            Self::Transcription(_) => DetectionMethod::SpeechTranscript,
            Self::Ner(_) => DetectionMethod::Ner,
            Self::ComputerVision(_) => DetectionMethod::ObjectDetection,
            Self::Translation(_) => DetectionMethod::ContextualNlp,
            Self::Classification(_) => DetectionMethod::ContextualNlp,
            Self::Summarization(_) => DetectionMethod::ContextualNlp,
        }
    }
}

/// Deterministic processing variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProcessingKind {
    /// Regex or dictionary matching.
    PatternMatch(ProcessingAction),
    /// Policy rule evaluation.
    PolicyEvaluation(ProcessingAction),
    /// Redaction application.
    Redaction(ProcessingAction),
    /// Data validation or schema check.
    Validation(ProcessingAction),
}

/// I/O lifecycle variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LifecycleKind {
    /// File ingest or load.
    Ingest(LifecycleAction),
    /// File publish or deliver.
    Publish(LifecycleAction),
    /// Content encryption.
    Encryption(LifecycleAction),
    /// Content compression.
    Compression(LifecycleAction),
    /// Format conversion (e.g. PDF to text, image resize).
    Conversion(LifecycleAction),
}

/// Top-level category for a [`FileAuditEntry`](super::FileAuditEntry).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "category", content = "action", rename_all = "snake_case")]
pub enum FileAuditEntryKind {
    /// AI-model inference operations.
    Inference(InferenceKind),
    /// Deterministic processing operations.
    Processing(ProcessingKind),
    /// I/O lifecycle operations.
    Lifecycle(LifecycleKind),
}
