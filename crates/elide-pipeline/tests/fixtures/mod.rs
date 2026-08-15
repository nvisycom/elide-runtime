//! Shared helpers for integration tests.

use std::path::PathBuf;

/// Write `bytes` to `tests/testdata/{stem}.{tag}` so a human
/// can open the artefact after a run. Gitignored.
///
/// `tag` is the full "suffix": everything after the stem's dot.
/// Callers pass e.g. `"out.docx"` for a redacted document,
/// `"audit.json"` for a JSON audit, `"audit-entities.csv"` for
/// a CSV table.
pub fn write_artefact(stem: &str, tag: &str, bytes: &[u8]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata")
        .join(format!("{stem}.{tag}"));
    std::fs::write(&path, bytes)
        .unwrap_or_else(|e| panic!("write artefact {}: {e}", path.display()));
}
