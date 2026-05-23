//! HuggingFace Hub model downloader.
//!
//! Available behind the `hf` feature.
//!
//! [`Downloader`] is a stateful fetcher that wraps `hf-hub`'s tokio
//! [`Api`] so callers can reuse one HTTP client across many downloads.
//! Designed to be shared across any crate that pulls model artifacts
//! from HuggingFace — NER backends, OCR models, embedding models, etc.
//!
//! Each [`Downloader::fetch`] call takes a [`FetchRequest`] (the
//! `(repo_id, revision, file, optional sha256)` tuple). Byte-level
//! download progress is reported automatically as throttled
//! `tracing::trace` events under the `nvisy_core::hf` target — no
//! explicit reporter to wire.
//!
//! # SHA-256 verification
//!
//! When [`FetchRequest::expected_sha256`] is `Some`, the downloaded
//! file is verified after the transfer completes via
//! [`FetchRequest::verify_artifact`]. The same method handles
//! externally-supplied artifacts (mirrors, manual downloads).
//!
//! [`Api`]: hf_hub::api::tokio::Api

mod downloader;
mod progress;
mod request;

pub use self::downloader::Downloader;
pub use self::request::FetchRequest;
