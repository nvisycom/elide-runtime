//! Recognition method classification with provenance metadata.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::provenance::{
    AnnotationProvenance, CrossReferenceProvenance, ModelKind, ModelProvenance, PatternKind,
    PatternProvenance,
};

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
    /// Text-pattern matching: regex, glob, dictionary lookup, or
    /// deny-list. The specific matcher lives in
    /// [`PatternProvenance::kind`].
    Pattern(PatternProvenance),
    /// Matching extracted values against an external identity or
    /// record database.
    CrossReference(CrossReferenceProvenance),
    /// Named-entity recognition via language model.
    Ner(ModelProvenance),
    /// Semantic similarity search via vector embeddings.
    Embedding(ModelProvenance),
    /// Pre-identified region supplied alongside the uploaded file.
    Annotation(AnnotationProvenance),
}

impl RecognitionMethod {
    /// Create a `Pattern` method tagged as a regex match.
    pub fn regex(pattern: impl Into<String>) -> Self {
        Self::Pattern(PatternProvenance {
            kind: PatternKind::Regex,
            pattern: Some(pattern.into()),
            validator: None,
            contextual: false,
        })
    }

    /// Create a `Pattern` method tagged as a regex match with a validator.
    pub fn regex_validated(pattern: impl Into<String>, validator: impl Into<String>) -> Self {
        Self::Pattern(PatternProvenance {
            kind: PatternKind::Regex,
            pattern: Some(pattern.into()),
            validator: Some(validator.into()),
            contextual: false,
        })
    }

    /// Create a `Pattern` method tagged as a dictionary match.
    pub fn dictionary(pattern: impl Into<String>) -> Self {
        Self::Pattern(PatternProvenance {
            kind: PatternKind::Dictionary,
            pattern: Some(pattern.into()),
            validator: None,
            contextual: false,
        })
    }

    /// Create a `Pattern` method tagged as a deny-list match. The
    /// pattern name is `None` because deny-list rules don't carry a
    /// named matcher.
    pub fn deny_list() -> Self {
        Self::Pattern(PatternProvenance {
            kind: PatternKind::DenyList,
            pattern: None,
            validator: None,
            contextual: false,
        })
    }

    /// Create a `CrossReference` method with the given source name.
    pub fn cross_reference(source: impl Into<String>) -> Self {
        Self::CrossReference(CrossReferenceProvenance {
            source: Some(source.into()),
        })
    }

    /// Create a `Ner` method with the given model name and kind.
    pub fn ner(name: impl Into<String>, kind: ModelKind) -> Self {
        Self::Ner(ModelProvenance::new(name, kind))
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
            Self::Pattern(_) => RecognitionMethodKind::Pattern,
            Self::CrossReference(_) => RecognitionMethodKind::CrossReference,
            Self::Ner(_) => RecognitionMethodKind::Ner,
            Self::Embedding(_) => RecognitionMethodKind::Embedding,
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
    Pattern,
    CrossReference,
    Ner,
    Embedding,
    Annotation,
}
