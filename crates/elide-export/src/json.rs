//! [`ExportJson`]: the whole-document format.
//!
//! Lossless and mechanical: everything the in-memory value holds,
//! nested as it holds it. Blanket implemented for every
//! [`Serialize`] type, so a type gains JSON export by deriving
//! serialization, with nothing to write here.
//!
//! The counterpart to [`ExportCsv`], which projects into flat
//! tables and loses what will not flatten.
//!
//! [`ExportCsv`]: crate::ExportCsv

use std::io;

use elide::Result;
use serde::Serialize;

use crate::export_failed;

/// Write this value as a JSON document.
///
/// Blanket implemented for every [`Serialize`] type; implementors
/// do not write this themselves.
pub trait ExportJson {
    /// Write compact JSON into `writer`.
    ///
    /// # Errors
    ///
    /// Returns [`Processing`](elide::ErrorKind::Processing) on a
    /// serialization or I/O failure.
    fn write_json<W: io::Write>(&self, writer: W) -> Result<()>;

    /// Write indented JSON into `writer`, for a human reader or a
    /// diffable artefact.
    ///
    /// # Errors
    ///
    /// Returns [`Processing`](elide::ErrorKind::Processing) on a
    /// serialization or I/O failure.
    fn write_json_pretty<W: io::Write>(&self, writer: W) -> Result<()>;

    /// This value as a compact JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`Processing`](elide::ErrorKind::Processing) on a
    /// serialization failure.
    fn to_json(&self) -> Result<String> {
        let mut buf = Vec::new();
        self.write_json(&mut buf)?;
        String::from_utf8(buf).map_err(|err| export_failed("JSON", err))
    }
}

impl<T: Serialize> ExportJson for T {
    fn write_json<W: io::Write>(&self, writer: W) -> Result<()> {
        serde_json::to_writer(writer, self).map_err(|err| export_failed("JSON", err))
    }

    fn write_json_pretty<W: io::Write>(&self, writer: W) -> Result<()> {
        serde_json::to_writer_pretty(writer, self).map_err(|err| export_failed("JSON", err))
    }
}
