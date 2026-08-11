//! [`BentoOcr`]: an [`OcrBackend`] backed by the
//! `nvisy-inference-ocr` BentoML service.
//!
//! Wire contract: `POST /recognize` accepts a batched list of
//! requests (each carrying base64-encoded image bytes + a
//! confidence threshold) and returns the matching list of
//! responses. Each response is a `Page -> Block -> Line -> Word`
//! tree; this backend flattens it to elide's
//! [`LayoutBlock`]/[`LayoutWord`] vocabulary: one [`LayoutBlock`]
//! per inference `Block`, every per-block word lifted into a
//! [`LayoutWord`] regardless of its parent `Line`. Per-call
//! correlation IDs propagate as `x-request-id` headers when set.
//!
//! Wire types live in the private `request` (outgoing) and
//! `response` (incoming) submodules; only the public
//! [`BentoOcr`] backend is part of this crate's API.
//!
//! [`OcrBackend`]: elide_ocr::OcrBackend
//! [`LayoutBlock`]: elide_core::modality::image::LayoutBlock
//! [`LayoutWord`]: elide_core::modality::image::LayoutWord

mod request;
mod response;

use bentoml::{Client, Endpoint};
use elide_core::Result;
use elide_core::entity::provenance::ModelEvent;
use elide_ocr::{OcrBackend, OcrRequest, OcrResponse};
use hipstr::HipStr;

use self::request::WireOcrRequest;
use self::response::WireOcrResponse;
use crate::error::BentoError;

const ROUTE: &str = "recognize";

/// BentoML OCR backend.
///
/// Owns a cached [`Endpoint`] pointing at the `nvisy-inference-ocr`
/// `/recognize` route, plus the per-deployment model id (echoed
/// into [`OcrBackend::provenance`]) and a default per-word
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
impl OcrBackend for BentoOcr {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn recognize(&self, request: OcrRequest<'_>) -> Result<OcrResponse> {
        let mut responses = self.recognize_batch(&[request]).await?;
        responses
            .pop()
            .ok_or_else(|| BentoError::Protocol("bento ocr returned an empty batch".into()).into())
    }

    async fn recognize_batch(&self, requests: &[OcrRequest<'_>]) -> Result<Vec<OcrResponse>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let responses = self.post_recognize(requests).await?;
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
