//! Tabular cell redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modality::{LeakProfile, RedactionStrategy};

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
    /// Drop the entire column containing this cell from the output.
    DropColumn,
    /// Drop the entire row containing this cell from the output.
    DropRow,
}

impl Default for TabularStrategy {
    fn default() -> Self {
        Self::Replace {
            placeholder: String::new(),
        }
    }
}

impl RedactionStrategy for TabularStrategy {
    /// - [`Hash`], [`Encrypt`] are [`Recoverable`].
    /// - [`Mask`], [`Replace`], [`Clear`] are [`Partial`] — the cell
    ///   still exists at known coordinates with an observable (empty
    ///   or masked) value.
    /// - [`DropColumn`], [`DropRow`] are [`Irrecoverable`] — the
    ///   entire column or row is gone schema-wide.
    ///
    /// [`Hash`]: Self::Hash
    /// [`Encrypt`]: Self::Encrypt
    /// [`Mask`]: Self::Mask
    /// [`Replace`]: Self::Replace
    /// [`Clear`]: Self::Clear
    /// [`DropColumn`]: Self::DropColumn
    /// [`DropRow`]: Self::DropRow
    /// [`Recoverable`]: LeakProfile::Recoverable
    /// [`Partial`]: LeakProfile::Partial
    /// [`Irrecoverable`]: LeakProfile::Irrecoverable
    fn leak_profile(&self) -> LeakProfile {
        match self {
            Self::Hash | Self::Encrypt { .. } => LeakProfile::Recoverable,
            Self::Mask { .. } | Self::Replace { .. } | Self::Clear => LeakProfile::Partial,
            Self::DropColumn | Self::DropRow => LeakProfile::Irrecoverable,
        }
    }
}
