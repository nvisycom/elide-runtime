//! Provenance metadata attached to recognition methods.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Provenance or licensing classification of a detection model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum ModelKind {
    /// Open-source model (e.g. spaCy, Hugging Face community models).
    OpenSource,
    /// Proprietary model (e.g. vendor-specific NER).
    Proprietary,
    /// Model accessed through a third-party API gateway.
    Gateway,
    /// Self-hosted model served behind an internal endpoint.
    SelfHosted,
}

/// Sub-classification within [`PatternProvenance::kind`] —
/// distinguishes the specific matcher that produced a
/// [`RecognitionMethod::Pattern`] detection.
///
/// [`RecognitionMethod::Pattern`]: super::recognition::RecognitionMethod::Pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum PatternKind {
    /// Regular expression matched against the full text.
    Regex,
    /// Exact-match lookup in a curated dictionary.
    Dictionary,
    /// Caller-supplied deny-list value forced into results.
    DenyList,
}

/// Provenance for a pattern-based detection (regex, dictionary,
/// deny-list). The matcher subtype lives in
/// [`PatternProvenance::kind`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PatternProvenance {
    /// Which matcher produced this detection.
    pub kind: PatternKind,
    /// Name of the pattern that matched (e.g. "ssn", "email"). May
    /// be absent for deny-list synthetic matches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Name of the validator that confirmed the match (e.g. "luhn",
    /// "iban"). Only meaningful when `kind == Regex`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
    /// Whether contextual analysis (keyword co-occurrence) adjusted
    /// the confidence score for this match.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub contextual: bool,
}

/// Provenance for a cross-reference detection — matching extracted
/// values against an external identity or record database.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CrossReferenceProvenance {
    /// Name of the cross-reference source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Provenance for a model-based detection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvenance {
    /// Model name (e.g. `"spacy-en-core-web-lg"`, `"gpt-4"`).
    pub name: String,
    /// Provenance / licensing classification.
    pub kind: ModelKind,
    /// Model version string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl ModelProvenance {
    /// Create a new model provenance with the given name and kind.
    pub fn new(name: impl Into<String>, kind: ModelKind) -> Self {
        Self {
            name: name.into(),
            kind,
            version: None,
        }
    }

    /// Set the model version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// Provenance for an annotation (pre-identified region from upload).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AnnotationProvenance {
    /// Identifier of the annotator (human or service account).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotator: Option<String>,
}
