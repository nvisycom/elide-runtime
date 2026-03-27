//! Two-level tagged enum discriminating audit entry categories.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::action::{InferenceAction, LifecycleAction, ProcessingAction};
use crate::entity::ExtractionMethod;

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
    /// Returns the [`ExtractionMethod`] for inference kinds that perform
    /// content extraction. Returns `None` for pure recognition or
    /// non-extraction operations.
    pub fn extraction_method(&self) -> Option<ExtractionMethod> {
        match self {
            Self::Ocr(_) => Some(ExtractionMethod::OpticalCharacterRecognition),
            Self::Transcription(_) => Some(ExtractionMethod::Transcription),
            Self::ComputerVision(_) => Some(ExtractionMethod::ObjectDetection),
            Self::Ner(_)
            | Self::Translation(_)
            | Self::Classification(_)
            | Self::Summarization(_) => None,
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
    /// File import or load.
    Import(LifecycleAction),
    /// File export or deliver.
    Export(LifecycleAction),
    /// Content encryption.
    Encryption(LifecycleAction),
    /// Content compression.
    Compression(LifecycleAction),
}

/// Top-level category for an [`AuditEntry`].
///
/// [`AuditEntry`]: super::AuditEntry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "category", content = "action", rename_all = "snake_case")]
pub enum AuditEntryKind {
    /// AI-model inference operations.
    Inference(InferenceKind),
    /// Deterministic processing operations.
    Processing(ProcessingKind),
    /// I/O lifecycle operations.
    Lifecycle(LifecycleKind),
}
