//! Tabular modality: `impl Codable for Tabular`, the [`TabularHandle`]
//! extension trait, plus concrete tabular format implementations
//! (CSV, XLSX).
//!
//! Tabular handlers address content by cell coordinate
//! ([`Tabular`] = row + column, optionally with intra-cell byte
//! offsets) and return [`TextData`] from `read` — the cell's string
//! value — so [`ModalityData::Data`] aliases [`TextData`] for the
//! [`Tabular`] modality. Replacements written during
//! [`IndexedHandle::redact`] use
//! [`nvisy_core::redaction::TabularReplacement`]; cells are strings,
//! so the per-format handlers share the text crate's redact helper.
//!
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact
//! [`Tabular`]: nvisy_core::modality::Tabular
//! [`TextData`]: nvisy_core::modality::TextData
//! [`ModalityData::Data`]: nvisy_core::modality::ModalityData::Data

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
pub trait TabularHandle: Handle<Tabular> {
    /// `true` when the source format carries explicit column headers
    /// or typed schema (CSV with header row, Parquet, XLSX); `false`
    /// when column semantics have to be inferred from the data.
    fn has_header(&self) -> bool;
}

#[cfg(feature = "csv")]
mod csv_handler;
#[cfg(feature = "csv")]
mod csv_loader;
#[cfg(feature = "xlsx")]
mod xlsx_handler;
#[cfg(feature = "xlsx")]
mod xlsx_loader;

#[cfg(feature = "csv")]
pub use self::csv_handler::{CsvData, CsvHandler, format as csv_format};
#[cfg(feature = "csv")]
pub use self::csv_loader::CsvLoader;
#[cfg(feature = "xlsx")]
pub use self::xlsx_handler::{XlsxHandler, format as xlsx_format};
#[cfg(feature = "xlsx")]
pub use self::xlsx_loader::XlsxLoader;
