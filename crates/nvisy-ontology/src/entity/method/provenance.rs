//! Provenance metadata attached to recognition methods.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::model::ModelInfo;

/// Provenance for a pattern-based detection (regex, dictionary, cross-reference).
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
