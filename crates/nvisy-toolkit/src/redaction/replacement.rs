//! Per-modality `Modality::Replacement` enums recorded on
//! `Execution::Applied`.
//!
//! For each modality, the replacement type captures *what the codec
//! wrote* at the entity's location. Text and Tabular carry the
//! substitution string (or "removed"); Image and Audio reuse the
//! existing per-modality method tag enums (the substitution is a
//! binary pixel/sample transform whose parameters live on the
//! producing `M::Strategy`, so the audit only needs to record *which*
//! operation ran).
//!
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the codec wrote at a text-modality entity's location.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextReplacement {
    /// The codec substituted the span with this string. Covers
    /// `TextStrategy::Mask`, `Replace`, `Hash`, `Encrypt` — every
    /// method that emits a string-shaped output.
    Substituted { value: String },
    /// The codec deleted the span entirely. Covers
    /// `TextStrategy::Remove`.
    Removed,
}

/// What the codec wrote at a tabular-modality entity's location.
///
/// Mirrors [`TextReplacement`] today; kept as a distinct type so
/// tabular-specific outcomes (e.g. "dropped column" telemetry) can
/// diverge without churning the text type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularReplacement {
    /// The codec wrote this string into the cell. Covers
    /// `TabularStrategy::Mask`, `Replace`, `Hash`, `Encrypt`, and
    /// `Clear` (Clear writes the empty string).
    Substituted { value: String },
    /// The codec dropped the column schema-wide. Covers
    /// `TabularStrategy::DropColumn`.
    ColumnDropped,
}
