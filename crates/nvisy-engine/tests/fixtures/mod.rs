//! Shared helpers for integration tests.

use std::path::PathBuf;

/// Write `bytes` to `tests/testdata/{stem}.out.{ext}` so a human
/// can open the redacted artefact after a run. Gitignored.
pub fn write_artefact(stem: &str, ext: &str, bytes: &[u8]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata")
        .join(format!("{stem}.out.{ext}"));
    std::fs::write(&path, bytes)
        .unwrap_or_else(|e| panic!("write artefact {}: {e}", path.display()));
}
