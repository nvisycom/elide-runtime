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

/// Provenance for a pattern-based detection (regex, dictionary,
/// deny-list). Each variant carries only the fields meaningful for
/// that matcher — the old flat `PatternKind` + `Option<String>`
/// representation allowed invalid combinations (a `Regex` row with
/// no pattern name, a `DenyList` row with a stale validator) that
/// can't be constructed in this shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PatternProvenance {
    /// Regular expression matched against the full text.
    Regex {
        /// Name of the pattern that matched (e.g. "ssn", "email").
        name: String,
        /// Name of the validator that confirmed the match (e.g.
        /// "luhn", "iban"), when one ran.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        validator: Option<String>,
        /// Whether contextual analysis (keyword co-occurrence)
        /// adjusted the confidence score for this match.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        contextual: bool,
    },
    /// Exact-match lookup in a curated dictionary.
    Dictionary {
        /// Name of the dictionary pattern that matched.
        name: String,
        /// Whether contextual analysis (keyword co-occurrence)
        /// adjusted the confidence score for this match.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        contextual: bool,
    },
    /// Caller-supplied deny-list value forced into results. Carries
    /// no per-match identity — the matched value is the same one the
    /// caller supplied.
    DenyList,
}

impl PatternProvenance {
    /// Mark this provenance as contextually adjusted. No-op for
    /// `DenyList`, which doesn't track contextual adjustment.
    pub fn mark_contextual(&mut self) {
        match self {
            Self::Regex { contextual, .. } | Self::Dictionary { contextual, .. } => {
                *contextual = true;
            }
            Self::DenyList => {}
        }
    }

    /// Pattern name that produced this match (regex or dictionary),
    /// or `None` for deny-list matches.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Regex { name, .. } | Self::Dictionary { name, .. } => Some(name),
            Self::DenyList => None,
        }
    }
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
    /// Name of the annotation as supplied by the uploader. Comes
    /// from [`Annotation::name`] at conversion time; downstream
    /// consumers use it as a human-readable label / source tag.
    ///
    /// [`Annotation::name`]: crate::entity::Annotation::name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
