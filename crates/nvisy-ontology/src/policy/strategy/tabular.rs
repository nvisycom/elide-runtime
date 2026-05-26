//! Tabular cell redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modality::RedactionStrategy;

const DEFAULT_MASK_CHAR: char = '*';

fn default_mask_char() -> char {
    DEFAULT_MASK_CHAR
}

/// Tabular cell redaction strategy.
///
/// Cells are text underneath, but the addressable unit is a cell —
/// whole-cell strategies dominate (drop the column, clear the cell)
/// alongside the same character-level strategies text supports.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TabularStrategy {
    /// Replace characters with a mask character.
    Mask {
        /// Character used for masking (default `'*'`).
        #[serde(default = "default_mask_char")]
        mask_char: char,
    },
    /// Substitute with a fixed placeholder string.
    Replace {
        /// Template for the replacement.
        #[serde(default)]
        placeholder: String,
    },
    /// Replace with a one-way hash.
    Hash,
    /// Encrypt the value; recoverable with the referenced key.
    Encrypt {
        /// Identifier of the encryption key to use.
        key_id: String,
    },
    /// Clear the cell value (set to empty).
    Clear,
    /// Drop the entire column from the output.
    DropColumn,
}

impl Default for TabularStrategy {
    fn default() -> Self {
        Self::Replace {
            placeholder: String::new(),
        }
    }
}

impl RedactionStrategy for TabularStrategy {
    fn is_reversible(&self) -> bool {
        matches!(self, Self::Encrypt { .. })
    }
}
