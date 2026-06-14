//! Provenance metadata attached to recognition methods.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Provenance for a pattern-based detection (regex, dictionary,
/// deny-list). Each variant carries only the fields meaningful for
/// that matcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PatternProvenance {
    /// Regular expression matched against the full text.
    Regex {
        /// Name of the pattern that matched (e.g. "ssn", "email").
        name: String,
        /// The actual regex string that matched, when the pattern
        /// engine surfaces it. Audit/compliance consumers store this
        /// to prove which regex triggered a redaction; recognizers
        /// that don't expose the pattern source leave it `None`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        regex: Option<String>,
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

/// Provenance for a model-based detection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvenance {
    /// Model name (e.g. `"spacy-en-core-web-lg"`, `"gpt-4"`).
    pub name: String,
    /// Model version string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether contextual analysis (keyword co-occurrence) adjusted
    /// the confidence score for this match. Mirrors the
    /// `contextual` flag on [`PatternProvenance`] so post-recognition
    /// enhancers can record their decision uniformly across
    /// pattern- and model-based detections.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub contextual: bool,
}

impl ModelProvenance {
    /// Create a new model provenance with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            contextual: false,
        }
    }

    /// Set the model version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Mark this provenance as contextually adjusted. Set by the
    /// `ContextEnhancer` when keyword co-occurrence boosted the
    /// match's confidence.
    pub fn mark_contextual(&mut self) {
        self.contextual = true;
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
