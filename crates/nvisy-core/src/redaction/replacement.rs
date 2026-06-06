//! Per-modality [`Modality::Replacement`] types — the rich,
//! byte-level instruction an anonymizer produces and the codec
//! consumes.
//!
//! These are not "audit tags" — they carry the full parameters the
//! codec needs to actually rewrite the underlying bytes. The audit
//! record stores the same value, so an audit replay reproduces the
//! redaction exactly.
//!
//! [`Modality::Replacement`]: super::Modality::Replacement

use bytes::Bytes;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitive::Color;

/// What a text-modality anonymizer writes at the entity's location.
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

    /// Borrow the replacement text. `Removed` answers `None`; the
    /// caller should treat that as the empty string (span deleted).
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Substituted { value } => Some(value),
            Self::Removed => None,
        }
    }
}

/// What an image-modality anonymizer writes at the entity's
/// location. Each variant carries the full parameter the codec
/// rasterizer needs.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageReplacement {
    /// Gaussian blur applied to the region with the given sigma.
    Blur { sigma: f32 },
    /// Opaque solid-colour block overlay on the region.
    Block { color: Color },
    /// Mosaic pixelation with a fixed block size in pixels.
    Pixelate { block_size: u32 },
    /// Region replaced with the supplied encoded image bytes.
    Replace { data: Bytes },
}

/// What a tabular-modality anonymizer writes at the entity's
/// location.
///
/// Tabular operators write a per-cell string or drop a column
/// outright. Per-column drops are recorded once per affected
/// column; downstream callers fan that single record out across
/// every row of the column.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularReplacement {
    /// Substitute the cell with this string.
    Substituted { value: String },
    /// Drop the column schema-wide.
    ColumnDropped,
}

impl TabularReplacement {
    /// Build a `Substituted` replacement.
    pub fn substituted(value: impl Into<String>) -> Self {
        Self::Substituted {
            value: value.into(),
        }
    }

    /// Borrow the cell's replacement text. `ColumnDropped` answers
    /// `None`; the caller should treat that as a schema-level drop
    /// signal, not as an empty-string substitution.
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Substituted { value } => Some(value),
            Self::ColumnDropped => None,
        }
    }
}

/// What an audio-modality anonymizer writes at the entity's
/// location. Each variant carries the parameter the codec needs to
/// produce the replacement samples.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioReplacement {
    /// The time-span is silenced.
    Silence,
    /// The time-span is removed (downstream samples shift left).
    Remove,
    /// The time-span is replaced with the supplied encoded audio
    /// bytes.
    Replace { data: Bytes },
}
