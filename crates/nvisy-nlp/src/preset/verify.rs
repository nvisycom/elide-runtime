//! Filesystem readability check for preset override paths.
//!
//! `check_readable` validates an explicit override path before the
//! engine tries to load the file, surfacing filesystem problems with
//! a clearer error than `ort` would. SHA-256 verification of model
//! artifacts lives in `nvisy_core::hf::verify_sha256` since it is
//! reused by the downloader.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_readable_accepts_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        check_readable(&path).unwrap();
    }

    #[test]
    fn check_readable_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let err = check_readable(dir.path()).unwrap_err();
        assert!(err.to_string().contains("not a regular file"));
    }

    #[test]
    fn check_readable_rejects_missing() {
        let err = check_readable(Path::new("/definitely/not/here/file.bin")).unwrap_err();
        assert!(err.to_string().contains("unreadable"));
    }
}
