//! [`FetchRequest`]: description of a single file to fetch from a
//! HuggingFace repo, plus its optional integrity hash.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// Description of a single file to fetch from a HuggingFace repo.
///
/// Bundles the three `&str`s that identify a file (`repo_id`,
/// `revision`, `file`) plus the optional integrity hash so call sites
/// stay self-documenting and the signature is stable as we extend it
/// (datasets, per-file user agents, etc.).
#[derive(Debug, Clone)]
pub struct FetchRequest<'a> {
    /// HuggingFace repo id (e.g. `"dslim/bert-base-NER"`).
    pub repo_id: &'a str,
    /// Commit SHA the artifact is pinned to. Required: artifacts
    /// must be content-addressed.
    pub revision: &'a str,
    /// Path within the repo (e.g. `"onnx/model.onnx"`).
    pub file: &'a str,
    /// Optional SHA-256 of the file contents, hex-encoded. When
    /// `Some`, [`verify_artifact`](Self::verify_artifact) checks the
    /// downloaded file against this hash.
    pub expected_sha256: Option<&'a str>,
}

impl<'a> FetchRequest<'a> {
    /// Construct a request without integrity verification.
    pub fn new(repo_id: &'a str, revision: &'a str, file: &'a str) -> Self {
        Self {
            repo_id,
            revision,
            file,
            expected_sha256: None,
        }
    }

    /// Builder-style: attach an expected SHA-256.
    #[must_use]
    pub fn with_sha256(mut self, expected_hex: &'a str) -> Self {
        self.expected_sha256 = Some(expected_hex);
        self
    }

    /// Verify a downloaded artifact at `path` against this request's
    /// `expected_sha256`. No-op when `expected_sha256` is `None`.
    /// Comparison is case-insensitive on the hex string.
    ///
    /// `Downloader::fetch` calls this automatically after a
    /// successful download; expose it for callers that obtain the
    /// file through some other channel (mirror, manual download,
    /// pre-staged image).
    pub fn verify_artifact(&self, path: &Path) -> Result<()> {
        let Some(expected) = self.expected_sha256 else {
            return Ok(());
        };
        let bytes = std::fs::read(path).map_err(|e| {
            Error::runtime(
                format!("read for sha256 {}: {e}", path.display()),
                "hf",
                false,
            )
        })?;
        let actual = hex(&Sha256::digest(&bytes));
        if actual.eq_ignore_ascii_case(expected) {
            Ok(())
        } else {
            Err(Error::runtime(
                format!(
                    "sha256 mismatch for {}: expected {expected}, got {actual}",
                    path.display(),
                ),
                "hf",
                false,
            ))
        }
    }
}

/// Hex-encode a byte slice into a lowercase `String`.
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
    fn builder_attaches_sha256() {
        let r = FetchRequest::new("dslim/bert-base-NER", "abc", "model.onnx");
        assert!(r.expected_sha256.is_none());
        let r = r.with_sha256("deadbeef");
        assert_eq!(r.expected_sha256, Some("deadbeef"));
    }

    #[test]
    fn verify_artifact_noop_without_hash() {
        let r = FetchRequest::new("x", "y", "z");
        r.verify_artifact(Path::new("/definitely/not/here"))
            .unwrap();
    }

    #[test]
    fn verify_artifact_accepts_matching_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let r = FetchRequest::new("x", "y", "z")
            .with_sha256("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
        r.verify_artifact(&path).unwrap();
    }

    #[test]
    fn verify_artifact_case_insensitive_hex() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        let r = FetchRequest::new("x", "y", "z")
            .with_sha256("2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824");
        r.verify_artifact(&path).unwrap();
    }

    #[test]
    fn verify_artifact_fails_on_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        let wrong = "0".repeat(64);
        let r = FetchRequest::new("x", "y", "z").with_sha256(&wrong);
        let err = r.verify_artifact(&path).unwrap_err();
        assert!(err.to_string().contains("sha256 mismatch"));
    }
}
