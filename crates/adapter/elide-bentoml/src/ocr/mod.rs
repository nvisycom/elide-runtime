//! [`BentoOcr`]: a [`Backend`] backed by the
//! `bento-doctr` BentoML service.
//!
//! Wire contract: `POST /recognize` accepts a batched list of
//! requests (each carrying base64-encoded image bytes + a
//! confidence threshold) and returns the matching list of
//! responses. Each response is a `Page -> Block -> Line -> Word`
//! tree; elide's [`Layout`] is a flat list of [`LayoutRegion`]s, so
//! this backend emits at the deepest level with content — words where
//! they exist, and the block or line itself where its children are
//! empty, which the contract permits. Per-call correlation IDs
//! propagate as `x-request-id` headers when set.
//!
//! Wire types live in the private `request` (outgoing) and
//! `response` (incoming) submodules; only the public
//! [`BentoOcr`] backend is part of this crate's API.
//!
//! [`Backend`]: elide_core::backend::Backend
//! [`Layout`]: elide_image::modality::Layout
//! [`LayoutRegion`]: elide_image::modality::LayoutRegion

mod request;
mod response;

use bentoml::{Client, Endpoint};
use elide_core::Result;
use elide_core::backend::Backend;
use elide_core::entity::audit::ModelEvent;
use elide_image::ocr::{OcrRequest, OcrResponse};
use hipstr::HipStr;

use self::request::WireOcrRequest;
use self::response::WireOcrResponse;
use crate::error::BentoError;

const ROUTE: &str = "recognize";

/// BentoML OCR backend.
///
/// Owns a cached [`Endpoint`] pointing at the `bento-doctr`
/// `/recognize` route, plus the per-deployment model id (echoed
/// into [`Backend::provenance`]) and a default per-word
/// confidence threshold (the service drops anything weaker before
/// returning).
#[derive(Debug, Clone)]
pub struct BentoOcr {
    /// Pre-built endpoint at the `/recognize` route. Cloned per
    /// call so per-request headers (`x-request-id`) layer onto a
    /// fresh instance without rebuilding the route.
    endpoint: Endpoint,
    /// Service-side model identifier echoed in provenance.
    model_id: HipStr<'static>,
    /// Default confidence floor sent on every request; the service
    /// drops weaker per-word recognitions before responding.
    default_threshold: f32,
}

impl BentoOcr {
    /// Build from a service URL + the deployment's model id.
    /// Default per-word confidence threshold is `0.0` (no
    /// filtering, matches the service's own default); use
    /// [`with_default_threshold`] to override.
    ///
    /// [`with_default_threshold`]: Self::with_default_threshold
    pub fn new(base_url: impl Into<String>, model_id: impl Into<HipStr<'static>>) -> Result<Self> {
        let client = Client::builder()
            .with_base_url(base_url)
            .build()
            .map_err(BentoError::Transport)?;
        Ok(Self {
            endpoint: client.endpoint(ROUTE),
            model_id: model_id.into(),
            default_threshold: 0.0,
        })
    }

    /// Override the per-request default per-word confidence
    /// threshold.
    #[must_use]
    pub fn with_default_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold;
        self
    }

    /// Send one batched `/recognize` POST end-to-end: encode
    /// each [`OcrRequest`] to its wire form, POST the batch
    /// (layering `x-request-id` when any request carries a
    /// correlation id), decode the wire responses back to
    /// [`OcrResponse`]s.
    async fn post_recognize(
        &self,
        requests: &[OcrRequest<'_>],
    ) -> Result<Vec<OcrResponse>, BentoError> {
        let body: Vec<WireOcrRequest> = requests
            .iter()
            .map(|r| WireOcrRequest::from_request(r, self.default_threshold))
            .collect();
        let mut endpoint = self.endpoint.clone();
        if let Some(id) = requests.iter().find_map(|r| r.correlation_id) {
            endpoint = endpoint.with_request_id(id.to_string());
        }
        let wire: Vec<WireOcrResponse> = endpoint
            .invoke(&body)
            .await
            .map_err(BentoError::Transport)?;
        Ok(wire.into_iter().map(WireOcrResponse::decode).collect())
    }
}

#[async_trait::async_trait]
impl Backend for BentoOcr {
    type Request<'a> = OcrRequest<'a>;
    type Response = OcrResponse;

    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn call(&self, request: OcrRequest<'_>) -> Result<OcrResponse> {
        // One request in, so exactly one response out. `pop` alone would
        // take the last of a longer list and hide the contract violation.
        let responses = self.post_recognize(&[request]).await?;
        let [response] = <[OcrResponse; 1]>::try_from(responses).map_err(|responses| {
            BentoError::Protocol(format!(
                "bento ocr returned {} responses for 1 request",
                responses.len(),
            ))
        })?;
        Ok(response)
    }

    /// Overridden: the `bento-doctr` service takes a batch in one POST, so the
    /// whole slice goes in a single round trip rather than the default's
    /// sequential fan-out over [`call`](Self::call).
    ///
    /// Responses come back in request order. An empty batch makes no call.
    async fn call_batch(&self, requests: Vec<OcrRequest<'_>>) -> Result<Vec<OcrResponse>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let responses = self.post_recognize(&requests).await?;
        if responses.len() != requests.len() {
            return Err(BentoError::Protocol(format!(
                "bento ocr returned {} responses for {} requests",
                responses.len(),
                requests.len(),
            ))
            .into());
        }
        Ok(responses)
    }
}
