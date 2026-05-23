//! Filesystem readability check for preset override paths.
//!
//! `check_readable` validates an explicit override path before the
//! engine tries to load the file, surfacing filesystem problems with
//! a clearer error than `ort` would. SHA-256 verification of model
//! artifacts lives on `nvisy_core::hf::FetchRequest::verify_artifact`
//! since it is reused by the downloader.

use std::path::Path;

use crate::error::{Error, Result};

/// Validate that `path` refers to a readable regular file.
///
/// Used to surface filesystem problems eagerly when an operator
/// supplies an explicit override path, before the engine tries to
/// load the file via `ort` and produces a less helpful error.
pub(super) fn check_readable(path: &Path) -> Result<()> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(Error::Backend(format!(
            "preset artifact is not a regular file: {}",
            path.display(),
        ))),
        Err(e) => Err(Error::Backend(format!(
            "preset artifact unreadable {}: {e}",
            path.display(),
        ))),
    }
}
