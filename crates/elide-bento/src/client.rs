//! Shared BentoML HTTP client wrapper.

use bentoml::Client;

use crate::error::BentoError;

/// Connection params common to every BentoML backend.
///
/// Per-modality backends (NER, OCR, …) typically wrap this with their
/// own per-endpoint config.
#[derive(Debug, Clone)]
pub struct BentoParams {
    /// Base URL of the BentoML service (e.g. `http://inference:3000`).
    pub base_url: String,
}

impl BentoParams {
    /// Construct, validating the URL parses.
    pub fn new(base_url: impl Into<String>) -> Result<Self, BentoError> {
        let base_url = base_url.into();
        url::Url::parse(&base_url).map_err(|e| BentoError::Config(format!("base_url: {e}")))?;
        Ok(Self { base_url })
    }
}

/// Thin wrapper over the upstream [`bentoml::Client`] for use by
/// per-modality backends. Owns the HTTP transport + base URL; the
/// modality-specific request/response wire shape lives in the
/// consuming crate.
#[derive(Debug, Clone)]
pub struct BentoClient {
    inner: Client,
}

impl BentoClient {
    /// Build from params. Returns a transport-ready client.
    pub fn new(params: &BentoParams) -> Result<Self, BentoError> {
        let inner = Client::builder()
            .with_base_url(&params.base_url)
            .build()
            .map_err(BentoError::Transport)?;
        Ok(Self { inner })
    }

    /// Borrow the underlying client for endpoint-specific calls.
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}
