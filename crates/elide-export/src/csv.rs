//! [`ExportCsv`]: the flat-table format.
//!
//! CSV cannot nest, so one value becomes several relations rather
//! than one document. Each is a named [`Table`] with its own
//! columns, and they join on [`Table::JOIN_KEY`].
//!
//! Unlike [`ExportJson`], there is no blanket impl: which columns
//! survive flattening is a judgement, not a derivation. Every
//! column dropped is loss, so it belongs in a named table with
//! documented columns.
//!
//! With the `zip` feature, [`ExportCsv::write_zip`] bundles every
//! table into one archive: the shape a caller wants when handing a
//! whole audit to a human or an HTTP download, where several files
//! beat one flattened table.
//!
//! [`ExportJson`]: crate::ExportJson

use std::{fmt, io};

use elide::Result;
use serde::Serialize;

use crate::export_failed;

/// One flat relation an audit projects into.
///
/// CSV cannot nest, so an audit becomes several tables rather than
/// one. Each is a named projection with its own columns, and they
/// join on [`JOIN_KEY`](Self::JOIN_KEY).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Table {
    /// One row per detected entity: what was found, where, and how
    /// confidently.
    Entities,
    /// One row per audit event, across every entity's provenance
    /// chain: how each detection came to exist and what happened to
    /// it.
    Provenance,
    /// One row per reviewer decision: which entities a human
    /// overrode, and how.
    Reviews,
}

impl Table {
    /// The column every table carries, so a caller can join them
    /// back together after export.
    pub const JOIN_KEY: &'static str = "entity_id";

    /// The table's name, as a file stem or a `--table` argument.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entities => "entities",
            Self::Provenance => "provenance",
            Self::Reviews => "reviews",
        }
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Write `header`, then one record per row, into `writer`.
///
/// The shared plumbing behind every [`ExportCsv`] impl: the header
/// is written even when `rows` is empty, so a table with no
/// results is an empty table rather than an empty file.
///
/// [`ExportCsv`]: crate::ExportCsv
///
/// # Errors
///
/// Returns [`Processing`](elide::ErrorKind::Processing) on a
/// serialization or I/O failure.
pub fn write_rows<W, R>(writer: W, header: &[&str], rows: impl IntoIterator<Item = R>) -> Result<()>
where
    W: io::Write,
    R: Serialize,
{
    let mut csv = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(writer);
    csv.write_record(header)
        .map_err(|err| export_failed("CSV", err))?;
    for row in rows {
        csv.serialize(row)
            .map_err(|err| export_failed("CSV", err))?;
    }
    csv.flush().map_err(|err| export_failed("CSV", err))
}

/// Write this value as one or more CSV tables.
///
/// A value that flattens into several relations implements this
/// once and names its tables in [`TABLES`](Self::TABLES); a caller
/// writes one by name, or iterates to write them all.
pub trait ExportCsv {
    /// The tables this value projects into, in the order a caller
    /// writing all of them would want.
    const TABLES: &'static [Table];

    /// Write `table` into `writer`, header first.
    ///
    /// The header is always written, so a table with no rows is an
    /// empty table rather than an empty file.
    ///
    /// # Errors
    ///
    /// Returns [`Processing`](elide::ErrorKind::Processing) on a
    /// serialization or I/O failure, or if `table` is not one this
    /// value offers.
    fn write_csv<W: io::Write>(&self, table: Table, writer: W) -> Result<()>;

    /// Bundle every table in [`TABLES`](Self::TABLES) into a zip
    /// archive, one `<table>.csv` entry each.
    ///
    /// What a caller wants when handing a whole audit to a human or
    /// an HTTP download: the tables stay separate relations instead
    /// of being flattened into one, and arrive as a single file.
    ///
    /// Entries are written in `TABLES` order and deflate-compressed;
    /// CSV is highly redundant, so the archive is typically a
    /// fraction of the concatenated files.
    ///
    /// # Errors
    ///
    /// Returns [`Processing`](elide::ErrorKind::Processing) on a
    /// serialization or I/O failure from any table, or from the
    /// archive itself.
    #[cfg(feature = "zip")]
    #[cfg_attr(docsrs, doc(cfg(feature = "zip")))]
    fn write_zip<W: io::Write + io::Seek>(&self, writer: W) -> Result<()> {
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut archive = zip::ZipWriter::new(writer);
        for table in Self::TABLES {
            archive
                .start_file(format!("{table}.csv"), options)
                .map_err(|err| export_failed("zip", err))?;
            // The writer is borrowed for one entry at a time, so
            // each table streams straight into the archive rather
            // than through an intermediate buffer.
            self.write_csv(*table, &mut archive)?;
        }
        archive.finish().map_err(|err| export_failed("zip", err))?;
        Ok(())
    }

    /// Every table as a zip archive in memory.
    ///
    /// Convenience over [`write_zip`](Self::write_zip) for a caller
    /// serving the bytes directly, such as an HTTP response body.
    ///
    /// # Errors
    ///
    /// As [`write_zip`](Self::write_zip).
    #[cfg(feature = "zip")]
    #[cfg_attr(docsrs, doc(cfg(feature = "zip")))]
    fn to_zip(&self) -> Result<Vec<u8>> {
        let mut buf = io::Cursor::new(Vec::new());
        self.write_zip(&mut buf)?;
        Ok(buf.into_inner())
    }
}

/// A value that can produce the rows of one table.
///
/// Split from [`ExportCsv`] so an implementor writes the
/// projection — the rows and their header — and inherits the
/// writer plumbing from [`write_rows`].
///
/// [`ExportCsv`]: crate::ExportCsv
pub trait TableRows {
    /// The row type this table serializes, one record per line.
    type Row<'a>: Serialize
    where
        Self: 'a;

    /// Column names, in order, matching `Row`'s field order.
    fn header() -> &'static [&'static str];

    /// The rows, in a stable order so two exports of one audit
    /// compare equal.
    fn rows(&self) -> Vec<Self::Row<'_>>;
}
