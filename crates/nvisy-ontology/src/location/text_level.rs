//! Hierarchical text-region levels for OCR results.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Hierarchical level of a text region within a document page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TextLevel {
    /// Full page.
    Page,
    /// Block-level region (paragraph, table, figure).
    Block,
    /// Single line of text.
    Line,
    /// Individual word.
    Word,
}
