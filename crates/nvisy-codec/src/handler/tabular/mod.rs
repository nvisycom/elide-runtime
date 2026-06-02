//! Tabular-modality wire types: [`Codable`] impl + redaction shape.
//!
//! Tabular handlers address content by cell coordinate
//! ([`Tabular`] = row + column, optionally with intra-cell byte
//! offsets), distinct from text handlers that address content by
//! byte offset in a serialized stream. The per-modality capability
//! surface lives on the generic [`Handle<Tabular>`] trait in
//! [`crate::core`]. Concrete per-format implementations (CSV, XLSX)
//! live in `nvisy-formats`; cells are strings, so they share the
//! text crate's redaction helper.
//!
//! Tabular handlers return [`TextData`] from `read` — the cell's
//! string value — so [`Codable::Data`] aliases [`TextData`] for the
//! [`Tabular`] modality.
//!
//! [`Handle<Tabular>`]: crate::core::Handle
//! [`Tabular`]: nvisy_core::modality::Tabular

use nvisy_core::modality::Tabular;

use super::TextData;
use crate::core::{Codable, Handle};

mod instruction;

pub use self::instruction::TabularRedaction;

impl Codable for Tabular {
    type Data = TextData;
    type Redaction = TabularRedaction;
}

/// Extension trait implemented by every tabular handler exposing the
/// "do I have a header row?" signal.
///
/// The engine importer reads this to pick
/// [`TabularExtraction::SchemaTyped`] when headers are known from the
/// source (header row in CSV, schema in Parquet/XLSX) vs.
/// [`TabularExtraction::SchemaInferred`] when column semantics must be
/// inferred from data.
///
/// Implementing this trait is required for every tabular handler that
/// participates in the importer fan-out.
///
pub trait TabularHandle: Handle<Tabular> {
    /// `true` when the source format carries explicit column headers
    /// or typed schema (CSV with header row, Parquet, XLSX); `false`
    /// when column semantics have to be inferred from the data.
    fn has_header(&self) -> bool;
}
