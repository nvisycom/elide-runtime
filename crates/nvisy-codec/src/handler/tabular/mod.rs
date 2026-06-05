//! Tabular-modality wire types: [`Codable`] impl + handler trait.
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
//! string value — so [`ModalityData::Data`] aliases [`TextData`] for
//! the [`Tabular`] modality. Replacements written during
//! [`IndexedHandle::redact`] use
//! [`nvisy_core::redaction::TabularReplacement`].
//!
//! [`ModalityData::Data`]: nvisy_core::modality::ModalityData::Data
//!
//! [`Handle<Tabular>`]: crate::core::Handle
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact
//! [`Tabular`]: nvisy_core::modality::Tabular
//! [`TextData`]: nvisy_core::modality::TextData

use nvisy_core::modality::{ModalityKind, Tabular};

use crate::core::{Codable, Handle};

impl Codable for Tabular {
    const KIND: ModalityKind = ModalityKind::Tabular;
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
/// [`TabularExtraction::SchemaTyped`]: nvisy_core::modality::TabularExtraction::SchemaTyped
/// [`TabularExtraction::SchemaInferred`]: nvisy_core::modality::TabularExtraction::SchemaInferred
///
/// Implementing this trait is required for every tabular handler that
/// participates in the importer fan-out.
pub trait TabularHandle: Handle<Tabular> {
    /// `true` when the source format carries explicit column headers
    /// or typed schema (CSV with header row, Parquet, XLSX); `false`
    /// when column semantics have to be inferred from the data.
    fn has_header(&self) -> bool;
}
