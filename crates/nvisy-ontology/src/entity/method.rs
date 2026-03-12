//! Extraction, recognition, and refinement method classification.
//!
//! These enums form the provenance record for every detected entity,
//! documenting how content was extracted from its source modality,
//! how sensitive data was identified, and what post-detection
//! refinements were applied.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// How content was extracted from its source modality into analyzable form.
///
/// Each variant names the technique that transformed raw content
/// (image pixels, audio samples, binary file formats) into a
/// representation suitable for entity recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ExtractionMethod {
    // Text
    /// Structural parsing of document formats (PDF, DOCX, HTML)
    /// into text and layout primitives.
    DocumentParsing,
    /// Inference of field semantics from column names, types, or
    /// positional conventions in tabular data.
    SchemaInference,

    // Image / Video
    /// Optical character recognition: converts raster text
    /// (printed or handwritten) into machine-readable characters.
    OpticalCharacterRecognition,
    /// Object detection: locates and labels regions of interest
    /// within an image or video frame.
    ObjectDetection,
    /// Scene text detection: localises text embedded in natural
    /// images (signs, screens, whiteboards) prior to OCR.
    SceneTextDetection,
    /// Table extraction: recovers row/column structure from images
    /// or scanned PDFs, preserving cell relationships that plain
    /// OCR loses.
    TableExtraction,
    /// Document layout analysis: identifies structural regions
    /// (headers, footers, signature blocks, form fields) by spatial
    /// arrangement rather than content.
    LayoutAnalysis,
    /// Metadata extraction: reads EXIF, PDF properties, or other
    /// embedded metadata that may contain PII (author, GPS, device info).
    MetadataExtraction,
    /// Frame extraction: samples individual frames from video
    /// streams for downstream image analysis.
    FrameExtraction,

    // Audio / Video
    /// Speech-to-text transcription: converts audio into text.
    Transcription,
    /// Speaker diarization: segments audio by speaker identity
    /// to attribute utterances before recognition.
    Diarization,
}

/// Technique used to identify a sensitive entity within extracted content.
///
/// Each variant names a self-contained recognition strategy.
/// An entity's `recognition_methods` vector records every technique
/// that contributed to its identification, ordered by application time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RecognitionMethod {
    // Pattern
    /// Regular expression matching against known PII formats.
    Regex,
    /// Mathematical validation of a candidate value
    /// (Luhn, IBAN mod-97, SSN area rules).
    Checksum,
    /// Exact-match lookup in a curated value list.
    Dictionary,
    /// Co-occurrence analysis: keywords near a candidate raise or
    /// lower confidence (e.g. "SSN" adjacent to a 9-digit number).
    ContextualAnalysis,
    /// Format heuristics: entropy, character distribution, or
    /// structural cues that suggest a value is sensitive without
    /// an explicit regex.
    Heuristic,

    // Model
    /// Named-entity recognition via language model.
    Ner,
    /// Document or field-level classification
    /// (e.g. "this column contains SSNs").
    Classification,
    /// Semantic similarity search via vector embeddings.
    Embedding,
    /// Matching extracted values against an external identity or
    /// record database.
    CrossReference,

    // Biometric
    /// Biometric identification: face recognition, voiceprint
    /// matching, or other physiological/behavioral trait analysis.
    Biometric,

    // Human
    /// User-provided annotation.
    Manual,
}

/// Post-detection refinement applied to an entity before final output.
///
/// Refinement methods do not discover new entities: they adjust
/// confidence, merge duplicates, or verify existing detections.
/// Recorded on the entity to explain why its final state may differ
/// from the initial detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RefinementMethod {
    /// Cross-detector deduplication: merges overlapping entities
    /// from independent detectors, combining their confidence and
    /// attribution.
    Deduplication,
    /// Ensemble fusion: combines confidence scores from multiple
    /// detectors using a voting or averaging strategy.
    EnsembleFusion,
    /// Model-based verification: a secondary model (typically VLM)
    /// reviews detections against source content to confirm, correct,
    /// or reject.
    ModelVerification,
    /// Policy evaluation: applies business rules, thresholds, or
    /// per-category overrides to filter or re-score detections.
    PolicyEvaluation,
    /// Human review: a reviewer confirmed, corrected, or rejected
    /// the detection.
    HumanReview,
    /// Confidence calibration: adjusts raw model scores to align
    /// with empirical precision targets.
    ConfidenceCalibration,
    /// Contextual promotion/demotion: surrounding document context
    /// upgrades or downgrades an entity's confidence after initial
    /// detection.
    ContextualAdjustment,
}
