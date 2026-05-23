//! [`Downloader`]: stateful HuggingFace Hub fetcher.
//!
//! Wraps `hf-hub`'s tokio [`Api`] so callers can reuse one HTTP client
//! across many downloads. Every download reports byte-level progress
//! via a private tracing-based reporter — events land on the
//! `nvisy_core::hf` tracing target, throttled to roughly one log line
//! per 5% of the file so logs stay readable on multi-hundred-megabyte
//! downloads.
//!
//! [`Api`]: hf_hub::api::tokio::Api

use std::path::PathBuf;

use hf_hub::api::tokio::{Api, ApiBuilder};
use hf_hub::{Cache, Repo, RepoType};

use super::FetchRequest;
use super::progress::TracingProgress;
use crate::{Error, Result};

/// Stateful HuggingFace Hub fetcher.
///
/// Owns one `hf-hub` API client so HTTP connections can be reused
/// across multiple [`fetch`](Self::fetch) calls — the same downloader
/// instance can pull the model and tokenizer (and additional files)
/// for one or more model presets.
///
/// Construct via [`Downloader::new`] for the default cache location
/// (`dirs::cache_dir()/nvisy/models/`), or [`Downloader::with_cache`]
/// for a custom directory.
pub struct Downloader {
    api: Api,
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
            .map_err(|e| Error::runtime(format!("hf-hub api init: {e}"), "hf", false))?;
        Ok(Self { api })
    }

    /// Fetch a single file described by `request`. Progress is
    /// reported as throttled `tracing::trace` events. When
    /// `request.expected_sha256` is `Some`, the file is verified
    /// after download (via [`FetchRequest::verify_artifact`]) and a
    /// mismatch returns an error.
    pub async fn fetch(&self, request: &FetchRequest<'_>) -> Result<PathBuf> {
        let path = self
            .api
            .repo(Repo::with_revision(
                request.repo_id.to_owned(),
                RepoType::Model,
                request.revision.to_owned(),
            ))
            .download_with_progress(request.file, TracingProgress::default())
            .await
            .map_err(|e| {
                Error::runtime(
                    format!(
                        "download {}/{}@{}: {e}",
                        request.repo_id, request.file, request.revision,
                    ),
                    "hf",
                    true,
                )
            })?;
        request.verify_artifact(&path)?;
        Ok(path)
    }
}
