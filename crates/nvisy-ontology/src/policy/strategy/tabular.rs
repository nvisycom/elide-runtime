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

/// Parameter-less tag for each [`TabularStrategy`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TabularMethodTag {
    /// Tag for [`TabularStrategy::Mask`].
    Mask,
    /// Tag for [`TabularStrategy::Replace`].
    Replace,
    /// Tag for [`TabularStrategy::Hash`].
    Hash,
    /// Tag for [`TabularStrategy::Encrypt`].
    Encrypt,
    /// Tag for [`TabularStrategy::Clear`].
    Clear,
    /// Tag for [`TabularStrategy::DropColumn`].
    DropColumn,
}

impl RedactionStrategy for TabularStrategy {
    type Tag = TabularMethodTag;

    /// - [`Hash`], [`Encrypt`] are [`Recoverable`].
    /// - [`Mask`], [`Replace`], [`Clear`] are [`Partial`] — the cell
    ///   still exists at known coordinates with an observable (empty
    ///   or masked) value.
    /// - [`DropColumn`] is [`Irrecoverable`] — the column is gone
    ///   schema-wide.
    ///
    /// [`Hash`]: Self::Hash
    /// [`Encrypt`]: Self::Encrypt
    /// [`Mask`]: Self::Mask
    /// [`Replace`]: Self::Replace
    /// [`Clear`]: Self::Clear
    /// [`DropColumn`]: Self::DropColumn
    /// [`Recoverable`]: LeakProfile::Recoverable
    /// [`Partial`]: LeakProfile::Partial
    /// [`Irrecoverable`]: LeakProfile::Irrecoverable
    fn leak_profile(&self) -> LeakProfile {
        match self {
            Self::Hash | Self::Encrypt { .. } => LeakProfile::Recoverable,
            Self::Mask { .. } | Self::Replace { .. } | Self::Clear => LeakProfile::Partial,
            Self::DropColumn => LeakProfile::Irrecoverable,
        }
    }

    fn method_tag(&self) -> Self::Tag {
        match self {
            Self::Mask { .. } => TabularMethodTag::Mask,
            Self::Replace { .. } => TabularMethodTag::Replace,
            Self::Hash => TabularMethodTag::Hash,
            Self::Encrypt { .. } => TabularMethodTag::Encrypt,
            Self::Clear => TabularMethodTag::Clear,
            Self::DropColumn => TabularMethodTag::DropColumn,
        }
    }
}
