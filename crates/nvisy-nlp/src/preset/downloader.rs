//! [`Downloader`]: stateful HuggingFace Hub fetcher with stage-level
//! progress reporting.
//!
//! Wraps `hf-hub`'s tokio [`Api`] so callers can reuse one HTTP client
//! across many downloads and subscribe to stage transitions via the
//! [`ProgressReporter`] trait.
//!
//! # Why stage-level only
//!
//! `hf-hub` 0.5 owns the underlying `reqwest` client and does not
//! expose byte-level download callbacks. We emit one event per major
//! stage — resolve, download, verify, done — which is enough to
//! distinguish "the download is making progress" from "something is
//! stuck." Byte-level progress would require us to bypass `hf-hub`
//! and reinvent its cache layer; not worth the surface area for a
//! handful of model files.
//!
//! [`Api`]: hf_hub::api::tokio::Api

use std::path::PathBuf;
use std::sync::Arc;

use hf_hub::api::tokio::{Api, ApiBuilder};
use hf_hub::{Cache, Repo, RepoType};

use crate::error::{Error, Result};

/// Stages emitted by [`Downloader::fetch`] through the configured
/// [`ProgressReporter`].
///
/// Stages fire in this order: `Resolving` → `Downloading` →
/// (`Verifying`)? → `Done`. `Verifying` is emitted only when the
/// caller passes an expected SHA-256 to [`Downloader::fetch`].
#[derive(Debug, Clone)]
pub enum DownloadStage {
    /// About to resolve the file in the HF cache or fetch it from the
    /// remote.
    Resolving {
        /// Path within the repo (e.g. `"onnx/model.onnx"`).
        file: String,
    },
    /// `hf-hub` is downloading or copying the file. We cannot
    /// report byte-level progress; this fires once just before the
    /// download begins.
    Downloading {
        /// Path within the repo.
        file: String,
    },
    /// The file is being SHA-256-verified against the expected hash.
    Verifying {
        /// Path within the repo.
        file: String,
    },
    /// The file is ready and verified (if a hash was supplied).
    Done {
        /// Path within the repo.
        file: String,
        /// Local cached path the file resolves to.
        path: PathBuf,
    },
}

/// Subscribe to [`DownloadStage`] events fired during
/// [`Downloader::fetch`]. Implementations must be `Send + Sync` so
/// the downloader can be shared across async tasks.
pub trait ProgressReporter: Send + Sync {
    /// Called once for each stage transition. Implementations should
    /// return quickly — long work in this callback delays the
    /// download path.
    fn on_stage(&self, stage: &DownloadStage);
}

/// A reporter that drops every event. Default when no progress
/// callback is configured.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn on_stage(&self, _stage: &DownloadStage) {}
}

/// A reporter that forwards every stage to `tracing::info`. Useful
/// for operators who just want startup events in their logs without
/// implementing the trait themselves.
#[derive(Debug, Clone, Copy, Default)]
pub struct TracingReporter;

impl ProgressReporter for TracingReporter {
    fn on_stage(&self, stage: &DownloadStage) {
        match stage {
            DownloadStage::Resolving { file } => {
                tracing::info!(target: "nvisy_nlp::download", file, "resolving");
            }
            DownloadStage::Downloading { file } => {
                tracing::info!(target: "nvisy_nlp::download", file, "downloading");
            }
            DownloadStage::Verifying { file } => {
                tracing::info!(target: "nvisy_nlp::download", file, "verifying");
            }
            DownloadStage::Done { file, path } => {
                tracing::info!(
                    target: "nvisy_nlp::download",
                    file,
                    path = %path.display(),
                    "ready"
                );
            }
        }
    }
}

/// Stateful HuggingFace Hub fetcher.
///
/// Owns one `hf-hub` API client so HTTP connections can be reused
/// across multiple [`fetch`](Self::fetch) calls — the same downloader
/// instance can pull the model and tokenizer (and additional files)
/// for one or more presets.
///
/// Construct via [`Downloader::new`] for the default cache location
/// (`dirs::cache_dir()/nvisy/models/`), or [`Downloader::with_cache`]
/// for a custom directory. Attach progress reporting with
/// [`with_reporter`](Self::with_reporter).
pub struct Downloader {
    api: Api,
    reporter: Arc<dyn ProgressReporter>,
}

impl Downloader {
    /// Create a downloader using the default cache directory.
    pub fn new() -> Result<Self> {
        let cache = match dirs::cache_dir() {
            Some(dir) => Cache::new(dir.join("nvisy").join("models")),
            None => Cache::default(),
        };
        Self::with_cache(cache)
    }

    /// Create a downloader using an explicit cache.
    pub fn with_cache(cache: Cache) -> Result<Self> {
        let api = ApiBuilder::from_cache(cache)
            .build()
            .map_err(|e| Error::Backend(format!("hf-hub api init: {e}")))?;
        Ok(Self {
            api,
            reporter: Arc::new(NoopReporter),
        })
    }

    /// Attach a progress reporter. Replaces any previously configured
    /// reporter. Wrap in `Arc` outside if you need to share the
    /// reporter with other consumers.
    pub fn with_reporter<R>(mut self, reporter: R) -> Self
    where
        R: ProgressReporter + 'static,
    {
        self.reporter = Arc::new(reporter);
        self
    }

    /// Fetch a single file from a HuggingFace model repo pinned to
    /// `revision`. The local cached path is returned. When
    /// `expected_sha256` is `Some`, the file is verified after
    /// download and a mismatch returns an error.
    pub async fn fetch(
        &self,
        repo_id: &str,
        revision: &str,
        file: &str,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf> {
        self.reporter.on_stage(&DownloadStage::Resolving {
            file: file.to_owned(),
        });

        // hf-hub treats hits-the-cache and hits-the-network the same
        // from the caller's perspective. We emit Downloading
        // unconditionally because we cannot tell which one it picked,
        // and the operator's main concern is "is the call in
        // progress, or am I stuck?".
        self.reporter.on_stage(&DownloadStage::Downloading {
            file: file.to_owned(),
        });
        let path = self
            .api
            .repo(Repo::with_revision(
                repo_id.to_owned(),
                RepoType::Model,
                revision.to_owned(),
            ))
            .get(file)
            .await
            .map_err(|e| Error::Backend(format!("download {repo_id}/{file}@{revision}: {e}")))?;

        if let Some(expected) = expected_sha256 {
            self.reporter.on_stage(&DownloadStage::Verifying {
                file: file.to_owned(),
            });
            super::verify::verify_sha256(&path, expected)?;
        }

        self.reporter.on_stage(&DownloadStage::Done {
            file: file.to_owned(),
            path: path.clone(),
        });
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Capturing reporter that records every stage it sees.
    #[derive(Default)]
    struct Capturing(Mutex<Vec<DownloadStage>>);

    impl ProgressReporter for Capturing {
        fn on_stage(&self, stage: &DownloadStage) {
            self.0.lock().unwrap().push(stage.clone());
        }
    }

    #[test]
    fn noop_reporter_does_nothing() {
        // Smoke: constructing and calling the noop reporter must not
        // panic and must compile under `ProgressReporter`.
        let reporter = NoopReporter;
        reporter.on_stage(&DownloadStage::Resolving {
            file: "model.onnx".to_owned(),
        });
    }

    #[test]
    fn tracing_reporter_emits_for_each_stage() {
        let reporter = TracingReporter;
        reporter.on_stage(&DownloadStage::Resolving {
            file: "m.onnx".to_owned(),
        });
        reporter.on_stage(&DownloadStage::Downloading {
            file: "m.onnx".to_owned(),
        });
        reporter.on_stage(&DownloadStage::Verifying {
            file: "m.onnx".to_owned(),
        });
        reporter.on_stage(&DownloadStage::Done {
            file: "m.onnx".to_owned(),
            path: PathBuf::from("/cache/m.onnx"),
        });
    }

    #[test]
    fn capturing_reporter_stores_events() {
        let reporter = Capturing::default();
        reporter.on_stage(&DownloadStage::Resolving {
            file: "x".to_owned(),
        });
        assert_eq!(reporter.0.lock().unwrap().len(), 1);
    }
}
