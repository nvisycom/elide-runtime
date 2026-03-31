//! Extraction, recognition, and refinement method classification.
//!
//! These enums form the provenance record for every detected entity,
//! documenting how content was extracted from its source modality,
//! how sensitive data was identified, and what post-detection
//! refinements were applied.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::model::ModelInfo;

/// How content was extracted from its source modality into analyzable form.
///
/// Each variant names the technique that transformed raw content
/// (image pixels, audio samples, binary file formats) into a
/// representation suitable for entity recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum ExtractionMethod {
    /// Structural parsing of document formats (PDF, DOCX, HTML)
    /// into text and layout primitives.
    DocumentParsing,
    /// Inference of field semantics from column names, types, or
    /// positional conventions in tabular data.
    SchemaInference,
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
    /// Speech-to-text transcription: converts audio into text.
    Transcription,
    /// Speaker diarization: segments audio by speaker identity
    /// to attribute utterances before recognition.
    Diarization,
}

/// Provenance for a pattern-based detection (regex, dictionary, checksum).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PatternProvenance {
    /// Name of the pattern that matched (e.g. "ssn", "email").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Name of the validator that confirmed the match (e.g. "luhn", "iban").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
    /// Whether contextual analysis (keyword co-occurrence) adjusted
    /// the confidence score for this match.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub contextual_analysis: bool,
}

/// Provenance for a model-based detection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ModelProvenance {
    /// The model that produced the detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelInfo>,
}

/// Provenance for a manual annotation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ManualProvenance {
    /// Identifier of the annotator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotator: Option<String>,
}

/// Technique used to identify a sensitive entity within extracted content.
///
/// Each variant names a self-contained recognition strategy and carries
/// optional provenance metadata about the specific tool that produced
/// the detection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecognitionMethod {
    // Pattern-based
    /// Regular expression matching against known PII formats.
    Regex(PatternProvenance),
    /// Exact-match lookup in a curated value list.
    Dictionary(PatternProvenance),
    /// Matching extracted values against an external identity or
    /// record database.
    CrossReference(PatternProvenance),

    // Model-based
    /// Named-entity recognition via language model.
    Ner(ModelProvenance),
    /// Document or field-level classification.
    Classification(ModelProvenance),
    /// Semantic similarity search via vector embeddings.
    Embedding(ModelProvenance),
    /// Biometric identification: face recognition, voiceprint
    /// matching, or other physiological/behavioral trait analysis.
    Biometric(ModelProvenance),

    // Human
    /// User-provided annotation.
    Manual(ManualProvenance),
}

impl RecognitionMethod {
    /// Create a `Regex` method with the given pattern name.
    pub fn regex(pattern: impl Into<String>) -> Self {
        Self::Regex(PatternProvenance {
            pattern: Some(pattern.into()),
            validator: None,
            contextual_analysis: false,
        })
    }

    /// Create a `Regex` method with pattern name and validator.
    pub fn regex_validated(pattern: impl Into<String>, validator: impl Into<String>) -> Self {
        Self::Regex(PatternProvenance {
            pattern: Some(pattern.into()),
            validator: Some(validator.into()),
            contextual_analysis: false,
        })
    }

    /// Create a `Dictionary` method with the given dictionary name.
    pub fn dictionary(pattern: impl Into<String>) -> Self {
        Self::Dictionary(PatternProvenance {
            pattern: Some(pattern.into()),
            validator: None,
            contextual_analysis: false,
        })
    }

    /// Create a `CrossReference` method with the given source name.
    pub fn cross_reference(pattern: impl Into<String>) -> Self {
        Self::CrossReference(PatternProvenance {
            pattern: Some(pattern.into()),
            validator: None,
            contextual_analysis: false,
        })
    }

    /// Create a `Ner` method with the given model info.
    pub fn ner(model: ModelInfo) -> Self {
        Self::Ner(ModelProvenance { model: Some(model) })
    }

    /// Create a `Classification` method with the given model info.
    pub fn classification(model: ModelInfo) -> Self {
        Self::Classification(ModelProvenance { model: Some(model) })
    }

    /// Create a `Biometric` method with the given model info.
    pub fn biometric(model: ModelInfo) -> Self {
        Self::Biometric(ModelProvenance { model: Some(model) })
    }

    /// Create a `Manual` method with the given annotator ID.
    pub fn manual(annotator: impl Into<String>) -> Self {
        Self::Manual(ManualProvenance {
            annotator: Some(annotator.into()),
        })
    }

    /// Returns the discriminant kind, stripping provenance data.
    /// Useful as a HashMap key when provenance details don't matter
    /// (e.g. calibration maps, weight tables).
    pub fn kind(&self) -> RecognitionMethodKind {
        match self {
            Self::Regex(_) => RecognitionMethodKind::Regex,
            Self::Dictionary(_) => RecognitionMethodKind::Dictionary,
            Self::CrossReference(_) => RecognitionMethodKind::CrossReference,
            Self::Ner(_) => RecognitionMethodKind::Ner,
            Self::Classification(_) => RecognitionMethodKind::Classification,
            Self::Embedding(_) => RecognitionMethodKind::Embedding,
            Self::Biometric(_) => RecognitionMethodKind::Biometric,
            Self::Manual(_) => RecognitionMethodKind::Manual,
            _ => RecognitionMethodKind::Regex,
        }
    }
}

/// Discriminant of [`RecognitionMethod`] without provenance data.
///
/// Used as a lightweight key for calibration maps and weight tables
/// where the specific pattern/model identity doesn't matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum RecognitionMethodKind {
    Regex,
    Dictionary,
    Ner,
    Classification,
    Embedding,
    CrossReference,
    Biometric,
    Manual,
}

/// Post-detection refinement applied to an entity before final output.
///
/// Refinement methods do not discover new entities: they adjust
/// confidence, merge duplicates, or verify existing detections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum RefinementMethod {
    /// Cross-detector deduplication.
    Deduplication,
    /// Ensemble fusion: combines confidence scores from multiple detectors.
    EnsembleFusion,
    /// Model-based verification: a secondary model reviews detections.
    ModelVerification,
    /// Policy evaluation: applies business rules or thresholds.
    PolicyEvaluation,
    /// Human review.
    HumanReview,
    /// Confidence calibration.
    ConfidenceCalibration,
    /// Contextual promotion/demotion.
    ContextualAdjustment,
}
