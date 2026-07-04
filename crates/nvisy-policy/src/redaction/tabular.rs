//! [`TabularRedaction`]: the operator spec a tabular-modality
//! policy rule carries.
//!
//! Tabular is `TextBacked` in elide (cells reuse the text
//! payload and replacement types), so cell-level ops are text
//! ops. The wire distinguishes the two shapes:
//!
//! - [`TabularRedaction::Cell`] wraps a [`TextRedaction`]. The
//!   operator runs on the matched cell's text content.
//! - [`TabularRedaction::DropRow`] / [`TabularRedaction::DropColumn`]
//!   are structural: they drop the row or column the matched
//!   cell sits in and have no text-level analogue.
//!
//! Future tabular-shape-aware operators (header-preserving
//! transforms, aggregation-safe redactions, per-column masking)
//! become sibling variants of `Cell` here without touching the
//! shared text vocabulary.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::text::TextRedaction;

/// Operator spec a `redact` tabular rule carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularRedaction {
    /// Run a text-modality operator on the matched cell's
    /// content. Cells are `TextBacked` in elide, so any
    /// [`TextRedaction`] applies unchanged.
    Cell {
        /// The text operator to apply. Reuses the full text
        /// vocabulary (Erase, Keep, Mask, Replace, Hash,
        /// Pseudonymize, Encrypt).
        spec: TextRedaction,
    },
    /// Drop the entire row a matched cell sits in.
    DropRow,
    /// Drop the entire column a matched cell sits in (header
    /// included).
    DropColumn,
}
