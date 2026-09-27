//! [`PioneerNer`]: a [`NerBackend`] backed by Pioneer's hosted GLiNER2
//! API.
//!
//! [`NerBackend`]: elide_ner::backend::NerBackend

mod request;
mod response;

use std::fmt;

use async_trait::async_trait;
use elide_core::Result;
use elide_core::entity::audit::ModelEvent;
use elide_core::primitive::LanguageTag;
use elide_ner::backend::{NerBackend, NerRequest, NerResponse};
use hipstr::HipStr;

use self::request::WireRequest;
use self::response::WireResponse;
use crate::error::PioneerError;

/// Pioneer's public API root.
const DEFAULT_BASE_URL: &str = "https://api.pioneer.ai";

/// The GLiNER2 extraction route.
const ROUTE: &str = "gliner-2";

/// Model identifier reported in provenance.
///
/// The dedicated `/gliner-2` route serves Pioneer's default GLiNER2 model
/// and takes no model selector, so this names the route rather than a
/// specific set of weights — the honest thing to record when the provider
/// can change them underneath us.
const MODEL_ID: &str = "pioneer/gliner-2";

/// Default per-label confidence cutoff, matching the Python client's.
const DEFAULT_THRESHOLD: f32 = 0.5;

/// A [`NerBackend`] backed by Pioneer's hosted GLiNER2 API.
///
/// # Where the text goes
///
/// **This sends the text to be scanned to a third party**, which for a
/// redaction pipeline is the un-redacted original. The self-hosted
/// `bento-gliner2` service runs the same open-weight model with no
/// egress.
///
/// Pioneer's published terms make that a live concern rather than a
/// theoretical one: retention is indefinite by default, training on
/// submitted data is on by default with no documented way to opt out
/// entirely, and no Data Processing Addendum is offered. See the crate
/// README before pointing this at production text.
///
/// [`NerBackend`]: elide_ner::backend::NerBackend
#[derive(Clone)]
pub struct PioneerNer {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    threshold: f32,
    zero_retention: bool,
}

/// Hand-written so the API key cannot reach a log or a panic message.
///
/// The derived implementation would print it verbatim, and a backend is
/// exactly the kind of value that ends up in a `tracing` field or an
/// `unwrap` diagnostic.
impl fmt::Debug for PioneerNer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PioneerNer")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("threshold", &self.threshold)
            .field("zero_retention", &self.zero_retention)
            .finish_non_exhaustive()
    }
}

impl PioneerNer {
    /// Build from an API key, against Pioneer's public endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be built.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    /// Build against a specific API root.
    ///
    /// The `gliner2` Python client still defaults to `api.fastino.ai`,
    /// whose certificate has expired; the live endpoint is
    /// `api.pioneer.ai`. This exists for a regional or self-hosted
    /// deployment, should one become available.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP client cannot be built.
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .build()
                .map_err(PioneerError::from)?,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            threshold: DEFAULT_THRESHOLD,
            zero_retention: false,
        })
    }

    /// Set the per-label confidence cutoff. Defaults to `0.5`.
    #[must_use]
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Send `store: false`, asking Pioneer not to persist the request or
    /// its response.
    ///
    /// Off by default, so a deployment gets Pioneer's own behaviour unless
    /// it opts in. Their documentation scopes zero-retention to "eligible
    /// use cases" without saying which, and it does not cover the
    /// task-model training their Trust & Safety page describes as
    /// continuing regardless — so treat this as a request rather than a
    /// guarantee, and see the crate README.
    #[must_use]
    pub fn with_zero_retention(mut self) -> Self {
        self.zero_retention = true;
        self
    }
}

#[async_trait]
impl NerBackend for PioneerNer {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: HipStr::borrowed(MODEL_ID),
            version: None,
            contextual: false,
        }
    }

    async fn recognize(&self, request: NerRequest<'_>) -> Result<NerResponse> {
        // Zero-shot: the labels arrive per call. Without them there is no
        // schema to send, and Pioneer's route requires one.
        //
        // The localized display name rather than the catalog id: GLiNER2
        // reads the label text semantically, so "email address" extracts
        // better than "EMAIL_ADDRESS". Pioneer's schema is a list of bare
        // names, so the description elide can carry has nowhere to go.
        let english = LanguageTag::english();
        let language = request.language.unwrap_or(&english);
        let labels: Vec<String> = request
            .labels
            .unwrap_or_default()
            .iter()
            .map(|label| label.name(language).to_owned())
            .collect();
        if labels.is_empty() {
            return Ok(NerResponse::new(Vec::new()));
        }

        let body = WireRequest::new(request.text, labels, self.threshold, self.zero_retention);
        let response = self
            .http
            .post(format!("{}/{ROUTE}", self.base_url))
            .header("X-API-Key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(PioneerError::from)?;

        let status = response.status();
        if !status.is_success() {
            // Pioneer explains refusals in the body — a bad key, an
            // unacceptable schema — so carry it rather than a bare code.
            let body = response.text().await.unwrap_or_default();
            return Err(PioneerError::Rejected {
                status: status.as_u16(),
                body,
            }
            .into());
        }

        let bytes = response.bytes().await.map_err(PioneerError::from)?;
        let wire: WireResponse = serde_json::from_slice(&bytes).map_err(|err| {
            PioneerError::Protocol(format!("could not decode the response body: {err}"))
        })?;

        Ok(wire.decode(request.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The API key must not reach a log line or a panic message.
    ///
    /// Pinned because the fix is a hand-written `Debug`: re-deriving it
    /// would silently print the credential again.
    #[test]
    fn debug_redacts_the_api_key() {
        let backend = PioneerNer::new("pio_sk_secret_value").expect("client");
        let rendered = format!("{backend:?}");

        assert!(!rendered.contains("pio_sk_secret_value"));
        assert!(rendered.contains("<redacted>"));
    }
}
