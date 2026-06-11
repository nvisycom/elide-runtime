//! Tabular modality: the [`TabularHandle`] extension trait plus
//! concrete tabular format implementations (CSV, XLSX).
//!
//! Tabular handlers address content by cell coordinate
//! ([`Tabular`] = row + column, optionally with intra-cell byte
//! offsets) and return [`TextData`] from `read` — the cell's string
//! value — so [`Modality::Data`] aliases [`TextData`] for the
//! [`Tabular`] modality. Replacements written during
//! [`Handler::redact`] use [`TabularReplacement`]; cells are
//! strings, so the per-format handlers share the text crate's redact
//! helper.
//!
//! [`Handler::redact`]: crate::Handler::redact
//! [`Tabular`]: nvisy_core::modality::Tabular
//! [`TabularReplacement`]: nvisy_core::redaction::TabularReplacement
//! [`TextData`]: nvisy_core::modality::TextData
//! [`Modality::Data`]: nvisy_core::modality::Modality::Data

use nvisy_core::modality::Tabular;

use crate::Handler;

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
pub trait TabularHandle: Handler<Tabular> {
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
