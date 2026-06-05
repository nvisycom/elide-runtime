//! Per-modality `Redactable::Replacement` types — what an
//! [`Anonymizer<M>`] writes at the entity's location.
//!
//! [`Anonymizer<M>`]: super::Anonymizer

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a text-modality [`Anonymizer<Text>`] writes at the entity's
/// location.
///
/// [`Anonymizer<Text>`]: super::Anonymizer
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextReplacement {
    /// Substitute the span with this string. Covers `Replace`,
    /// `Mask`, `Hash`, `Encrypt`, and every other text operator
    /// that emits a string.
    Substituted { value: String },
    /// Delete the span entirely. Covers `Redact`.
    Removed,
}

impl TextReplacement {
    /// Build a `Substituted` replacement.
    pub fn substituted(value: impl Into<String>) -> Self {
        Self::Substituted {
            value: value.into(),
        }
    }
}

/// What an image-modality [`Anonymizer<Image>`] writes at the
/// entity's location.
///
/// The image replacement is described by *which* binary pixel
/// transform ran (`Blur`, `Pixelate`, `BlackBox`, …); the parameter
/// values that produced it live on the operator instance. The
/// document phase consults the operator at apply time; the audit
/// only needs the method tag.
///
/// [`Anonymizer<Image>`]: super::Anonymizer
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageReplacement {
    /// Gaussian blur was applied to the region.
    Blur,
    /// The region was pixelated with a fixed block size.
    Pixelate,
    /// The region was filled with a solid colour.
    BlackBox,
}

/// What a tabular-modality [`Anonymizer<Tabular>`] writes at the
/// entity's location.
///
/// Tabular operators write a per-cell string or drop a column
/// outright. Per-column drops are recorded as `ColumnDropped` so the
/// audit row can distinguish "wrote `""`" from "schema-level drop."
///
/// [`Anonymizer<Tabular>`]: super::Anonymizer
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularReplacement {
    /// Substitute the cell with this string.
    Substituted { value: String },
    /// Drop the column schema-wide. Recorded once per affected
    /// column; downstream callers fan that single record out across
    /// every row of the column.
    ColumnDropped,
}

impl TabularReplacement {
    /// Build a `Substituted` replacement.
    pub fn substituted(value: impl Into<String>) -> Self {
        Self::Substituted {
            value: value.into(),
        }
    }
}

/// What an audio-modality [`Anonymizer<Audio>`] writes at the
/// entity's location.
///
/// The audio replacement is described by *which* binary
/// sample-mutation operator ran (`Silence`, `WhiteNoise`,
/// `Beep`, …); the actual replacement bytes are produced by the
/// format handler at apply time. The audit only needs the method
/// tag.
///
/// [`Anonymizer<Audio>`]: super::Anonymizer
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioReplacement {
    /// The time-span was silenced.
    Silence,
    /// The time-span was overlaid with white noise.
    WhiteNoise,
    /// The time-span was replaced with a single tone (beep).
    Beep,
}
