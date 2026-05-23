//! Filesystem + hash verification for preset artifacts.
//!
//! `check_readable` validates an explicit override path before the
//! engine tries to load the file, surfacing filesystem problems with
//! a clearer error than `ort` would. `verify_sha256` confirms a file
//! on disk matches the expected hex-encoded digest from a manifest;
//! it is feature-gated because the SHA-256 implementation pulls in
//! `sha2`, which we only want optional.

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

/// Verify a file on disk matches the expected hex-encoded SHA-256.
/// Comparison is case-insensitive on the hex string.
#[cfg(feature = "preset-download")]
#[cfg_attr(docsrs, doc(cfg(feature = "preset-download")))]
pub(super) fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> {
    use sha2::{Digest, Sha256};

    let bytes = std::fs::read(path)
        .map_err(|e| Error::Backend(format!("read for sha256 {}: {e}", path.display())))?;
    let actual = hex(&Sha256::digest(&bytes));
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(Error::Backend(format!(
            "sha256 mismatch for {}: expected {expected_hex}, got {actual}",
            path.display(),
        )))
    }
}

/// Hex-encode a byte slice into a lowercase `String`.
#[cfg(feature = "preset-download")]
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
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

    #[cfg(feature = "preset-download")]
    #[test]
    fn verify_sha256_accepts_matching_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        verify_sha256(
            &path,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        )
        .unwrap();
    }

    #[cfg(feature = "preset-download")]
    #[test]
    fn verify_sha256_rejects_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        let err = verify_sha256(&path, "0".repeat(64).as_str()).unwrap_err();
        assert!(err.to_string().contains("sha256 mismatch"));
    }
}
