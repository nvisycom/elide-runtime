//! Recognition method classification with provenance metadata.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::provenance::{AnnotationProvenance, ModelProvenance, PatternProvenance};
use crate::entity::model::ModelInfo;

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
    /// Pre-identified region supplied alongside the uploaded file.
    Annotation(AnnotationProvenance),
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

    /// Create an `Annotation` method with an optional annotator ID.
    pub fn annotation(annotator: Option<String>) -> Self {
        Self::Annotation(AnnotationProvenance { annotator })
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
            Self::Annotation(_) => RecognitionMethodKind::Annotation,
        }
    }
}

/// Discriminant of [`RecognitionMethod`] without provenance data.
///
/// Used as a lightweight key for calibration maps and weight tables
/// where the specific pattern/model identity doesn't matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(strum::Display, strum::EnumString, Serialize, Deserialize, JsonSchema)]
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
    Annotation,
}
