//! Prebuilt NLP-engine presets, selectable from configuration.
//!
//! Two presets ship today:
//!
//! - [`NlpPreset::Default`]: no-op backend. Detects nothing. Used by
//!   tests and by deployments that wire detection through patterns
//!   and/or LLM only.
//! - [`NlpPreset::Manifest`]: load a JSON manifest describing a
//!   downloadable HuggingFace token-classification model. The
//!   manifest pins the repo + revision + label map; operators ship
//!   their own JSON files and reference them by path. A reference
//!   manifest for `dslim/bert-base-NER` lives at
//!   `crates/nvisy-nlp/presets/dslim-bert-base-NER.json`.
//!
//! Model downloads require the `hf` feature, which activates the
//! shared [`hf`] module. Without it, [`NlpPreset::Manifest`] still
//! works as long as both `model_path` and `tokenizer_path` overrides
//! are supplied.
//!
//! SHA-256 verification of downloaded or supplied artifacts is
//! delegated to [`FetchRequest::verify_artifact`].
//!
//! # Override pattern for air-gapped deployments
//!
//! Set `model_path` and `tokenizer_path` together to skip the
//! HuggingFace download entirely. The manifest's `model_sha256` /
//! `tokenizer_sha256` (when set) still validate the supplied files.
//!
//! ```no_run
//! use std::path::PathBuf;
//! use nvisy_nlp::NlpPreset;
//!
//! let preset = NlpPreset::Manifest {
//!     manifest_path: PathBuf::from("./presets/dslim-bert-base-NER.json"),
//!     model_path: Some(PathBuf::from("/srv/models/dslim/model.onnx")),
//!     tokenizer_path: Some(PathBuf::from("/srv/models/dslim/tokenizer.json")),
//! };
//! ```
//!
//! [`hf`]: nvisy_core::hf
//! [`FetchRequest::verify_artifact`]: nvisy_core::hf::FetchRequest::verify_artifact

mod builder;
mod manifest;

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::manifest::{BackendConfig, LabelMapEntry, PresetManifest};
use crate::NlpEngine;
use crate::error::{Error, Result};
use crate::language::LinguaLanguagePolicy;
use crate::ner::NoopNerBackend;

/// Prebuilt NLP-engine preset.
///
/// Picked from workflow config; the recognizer layer calls [`build`]
/// to materialise the corresponding [`NlpEngine`].
///
/// [`build`]: Self::build
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum NlpPreset {
    /// No-op engine — returns no entities. Backed by
    /// [`NoopNerBackend`]. Useful for tests and for deployments that
    /// detect via patterns / LLM only.
    ///
    /// [`NoopNerBackend`]: crate::ner::NoopNerBackend
    #[default]
    Default,

    /// Load a manifest-defined preset.
    ///
    /// `manifest_path` is the path to a JSON [`PresetManifest`].
    /// `model_path` and `tokenizer_path` are optional overrides; when
    /// absent (and the `hf` feature is enabled), the artifacts are
    /// downloaded from HuggingFace per the manifest's `repo_id` +
    /// `revision`. Both overrides must be supplied together or both
    /// omitted.
    Manifest {
        /// Path to the JSON manifest file.
        manifest_path: PathBuf,
        /// Optional local path to the model ONNX file (overrides the
        /// download).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_path: Option<PathBuf>,
        /// Optional local path to the tokenizer file (overrides the
        /// download).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tokenizer_path: Option<PathBuf>,
    },
}

impl NlpPreset {
    /// Construct the [`NlpEngine`] this preset selects. The returned
    /// engine is cheap to [`Clone`] (three refcount bumps); callers
    /// don't need to wrap it in `Arc` themselves.
    pub async fn build(&self) -> Result<NlpEngine> {
        match self {
            Self::Default => NlpEngine::builder()
                .with_ner_backend(NoopNerBackend)
                .with_language_policy(LinguaLanguagePolicy)
                .build()
                .map_err(|e| Error::Backend(e.to_string())),

            Self::Manifest {
                manifest_path,
                model_path,
                tokenizer_path,
            } => {
                let manifest = PresetManifest::from_file(manifest_path)?;
                let (model, tokenizer) =
                    resolve_paths(&manifest, model_path.as_deref(), tokenizer_path.as_deref())
                        .await?;
                builder::build_engine(manifest, model, tokenizer)
            }
        }
    }
}

/// Pick artifacts: either both overrides (no download, verify hashes
/// against supplied files) or auto-fetch both via the shared
/// `nvisy_core::hf::Downloader` (which verifies inside `fetch`).
async fn resolve_paths(
    manifest: &PresetManifest,
    model_path: Option<&Path>,
    tokenizer_path: Option<&Path>,
) -> Result<(PathBuf, PathBuf)> {
    match (model_path, tokenizer_path) {
        (Some(m), Some(t)) => {
            check_readable(m)?;
            check_readable(t)?;
            verify_supplied_hashes(manifest, m, t)?;
            Ok((m.to_owned(), t.to_owned()))
        }
        (None, None) => fetch_both(manifest).await,
        _ => Err(Error::Backend(
            "NlpPreset::Manifest: provide both model_path and tokenizer_path, or neither"
                .to_owned(),
        )),
    }
}

/// Validate that `path` refers to a readable regular file. Surfaces
/// filesystem problems eagerly so an operator-supplied override path
/// fails here with a clear error rather than deeper inside `ort` /
/// `gline-rs`.
fn check_readable(path: &Path) -> Result<()> {
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

/// Verify the user-supplied artifact paths against the manifest's
/// `model_sha256` / `tokenizer_sha256` (when set). Skipped silently
/// when no hash is recorded; warns and skips when the `hf` feature
/// is off so operators don't quietly skip verification they thought
/// was active.
#[cfg(feature = "hf")]
fn verify_supplied_hashes(
    manifest: &PresetManifest,
    model_path: &Path,
    tokenizer_path: &Path,
) -> Result<()> {
    use nvisy_core::hf::FetchRequest;

    FetchRequest {
        repo_id: &manifest.repo_id,
        revision: &manifest.revision,
        file: &manifest.model_file,
        expected_sha256: manifest.model_sha256.as_deref(),
    }
    .verify_artifact(model_path)
    .map_err(|e| Error::Backend(e.to_string()))?;
    FetchRequest {
        repo_id: &manifest.repo_id,
        revision: &manifest.revision,
        file: &manifest.tokenizer_file,
        expected_sha256: manifest.tokenizer_sha256.as_deref(),
    }
    .verify_artifact(tokenizer_path)
    .map_err(|e| Error::Backend(e.to_string()))?;
    Ok(())
}

#[cfg(not(feature = "hf"))]
fn verify_supplied_hashes(
    manifest: &PresetManifest,
    _model_path: &Path,
    _tokenizer_path: &Path,
) -> Result<()> {
    if manifest.model_sha256.is_some() || manifest.tokenizer_sha256.is_some() {
        tracing::warn!(
            target: "nvisy_nlp::preset",
            "manifest specifies sha256 hashes but the `hf` feature is \
             disabled; skipping hash verification"
        );
    }
    Ok(())
}

/// Download both artifacts via a fresh [`Downloader`]. Progress is
/// reported automatically as `tracing::info` events under the
/// `nvisy_core::hf` target.
///
/// [`Downloader`]: nvisy_core::hf::Downloader
#[cfg(feature = "hf")]
async fn fetch_both(manifest: &PresetManifest) -> Result<(PathBuf, PathBuf)> {
    use nvisy_core::hf::{Downloader, FetchRequest};

    let downloader = Downloader::new().map_err(|e| Error::Backend(e.to_string()))?;
    let model = downloader
        .fetch(&FetchRequest {
            repo_id: &manifest.repo_id,
            revision: &manifest.revision,
            file: &manifest.model_file,
            expected_sha256: manifest.model_sha256.as_deref(),
        })
        .await
        .map_err(|e| Error::Backend(e.to_string()))?;
    let tokenizer = downloader
        .fetch(&FetchRequest {
            repo_id: &manifest.repo_id,
            revision: &manifest.revision,
            file: &manifest.tokenizer_file,
            expected_sha256: manifest.tokenizer_sha256.as_deref(),
        })
        .await
        .map_err(|e| Error::Backend(e.to_string()))?;
    Ok((model, tokenizer))
}

#[cfg(not(feature = "hf"))]
async fn fetch_both(_manifest: &PresetManifest) -> Result<(PathBuf, PathBuf)> {
    Err(Error::Backend(
        "NlpPreset::Manifest requires the `hf` feature to fetch \
         artifacts, or both model_path and tokenizer_path must be \
         supplied explicitly"
            .to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_artifact_path_errors_clearly() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("preset.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "model_name": "x", "repo_id": "x/y", "revision": "abc",
                "model_file": "m.onnx", "tokenizer_file": "t.json",
                "backend": {
                    "kind": "onnx-bert",
                    "id_to_label": ["O"], "label_map": {}
                }
            }"#,
        )
        .unwrap();
        let preset = NlpPreset::Manifest {
            manifest_path,
            model_path: Some(PathBuf::from("/definitely/not/here.onnx")),
            tokenizer_path: Some(PathBuf::from("/definitely/not/here.json")),
        };
        let err = preset.build().await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unreadable") || msg.contains("not a regular file"),
            "expected clear filesystem error, got: {msg}",
        );
    }

    #[tokio::test]
    async fn mixed_override_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("preset.json");
        std::fs::write(
            &manifest_path,
            r#"{
                "model_name": "x", "repo_id": "x/y", "revision": "abc",
                "model_file": "m.onnx", "tokenizer_file": "t.json",
                "backend": {
                    "kind": "onnx-bert",
                    "id_to_label": ["O"], "label_map": {}
                }
            }"#,
        )
        .unwrap();
        let preset = NlpPreset::Manifest {
            manifest_path,
            model_path: Some(PathBuf::from("/x")),
            tokenizer_path: None,
        };
        let err = preset.build().await.unwrap_err();
        assert!(
            err.to_string()
                .contains("both model_path and tokenizer_path"),
        );
    }
}
