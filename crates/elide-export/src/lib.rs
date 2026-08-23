#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Export formats for `elide-runtime` audits.
//!
//! Two formats, two traits, because they answer different
//! questions:
//!
//! - [`ExportJson`] writes one document, whole. Every field, every
//!   entity's provenance chain, nested as it is in memory. The
//!   canonical export: what a caller reaches for to persist an
//!   audit and post it back later.
//! - [`ExportCsv`] writes *tables*. CSV has no nesting, so one
//!   audit becomes several flat relations that join on
//!   [`Table::JOIN_KEY`]. A caller picks one table, iterates
//!   [`ExportCsv::TABLES`] to write them all, or bundles them into
//!   a zip archive.
//!
//! Each format is self-contained in its own module behind its own
//! feature, so a caller who wants only one pays for only one.

use elide::{Error, ErrorKind};

#[cfg(feature = "csv")]
mod csv;
#[cfg(feature = "json")]
mod json;

#[cfg(feature = "csv")]
#[cfg_attr(docsrs, doc(cfg(feature = "csv")))]
pub use self::csv::{ExportCsv, Table, TableRows, write_rows};
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use self::json::ExportJson;

/// Wrap a failure as a [`Processing`](ErrorKind::Processing)
/// error, the kind every export failure surfaces as.
///
/// Shared by both formats so a caller matches one error kind
/// whichever they used.
pub(crate) fn export_failed(format: &str, err: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorKind::Processing,
        format!("{format} export failed: {err}"),
    )
}
