//! File-audit entry types.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use crate::entity::ModelInfo;

/// Classifies the activity recorded by a [`FileAuditEntry`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum FileAuditEntryKind {
    /// OCR text extraction.
    Ocr,
    /// Audio/video transcription.
    Transcription,
    /// Named-entity recognition.
    Ner,
    /// Face or object detection.
    ComputerVision,
    /// Regex or dictionary matching.
    PatternMatch,
    /// Policy rule evaluation.
    PolicyEvaluation,
    /// Redaction application.
    Redaction,
    /// Content or context translation.
    Translation,
    /// Content classification.
    Classification,
    /// Content summarization.
    Summarization,
    /// File ingest or load.
    Ingest,
    /// File publish or deliver.
    Publish,
}

/// A single processing-log entry within a [`FileAudit`](super::FileAudit).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileAuditEntry {
    /// What kind of operation was performed.
    pub kind: FileAuditEntryKind,
    /// When the operation occurred.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Model used, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelInfo>,
    /// Wall-clock duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Input tokens consumed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Number of items processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
    /// Human-readable description of the operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FileAuditEntry {
    /// Create a new audit entry with the given kind and current timestamp.
    pub fn new(kind: FileAuditEntryKind) -> Self {
        Self {
            kind,
            timestamp: Timestamp::now(),
            model: None,
            duration_ms: None,
            input_tokens: None,
            output_tokens: None,
            items_count: None,
            description: None,
        }
    }

    /// Set the model that performed this operation.
    pub fn with_model(mut self, model: ModelInfo) -> Self {
        self.model = Some(model);
        self
    }

    /// Set the wall-clock duration.
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Set the token counts.
    pub fn with_tokens(mut self, input: u64, output: u64) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self
    }
}
