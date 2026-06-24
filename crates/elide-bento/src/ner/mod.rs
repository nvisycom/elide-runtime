//! [`BentoNer`]: an [`elide_ner::backend::NerBackend`] backed by the
//! `nvisy-inference-ner` BentoML service.
//!
//! Wire contract: `POST /recognize` accepts a batched list of
//! requests (one schema-driven entity-extraction call per item)
//! and returns the matching list of responses. Each request carries
//! a schema (entities + optional classifications + structures); this
//! backend uses entities only and ignores the rest. Per-call
//! correlation IDs propagate as `x-request-id` headers when set.
//!
//! Layout:
//!
//! - [`mod@request`] — outgoing wire shapes (`WireNerRequest`,
//!   `WireSchema`, `WireEntitySpec`).
//! - [`mod@response`] — incoming wire shapes (`WireNerResponse`,
//!   `WireEntity`) plus the [`response::decode`] helper.

mod request;
mod response;

use bentoml::{Client, Endpoint};
use elide_core::Result;
use elide_core::entity::provenance::ModelEvent;
use elide_ner::backend::{NerBackend, NerRequest, NerResponse};
use hipstr::HipStr;

use self::request::WireNerRequest;
use self::response::WireNerResponse;
use crate::error::BentoError;

const ROUTE: &str = "recognize";

/// BentoML NER backend.
///
/// Owns a cached [`Endpoint`] pointing at the `nvisy-inference-ner`
/// `/recognize` route, plus the per-deployment model id (echoed
/// into [`NerBackend::provenance`]) and a default per-label
/// confidence threshold the service applies when a schema entry
/// does not pin its own.
#[derive(Debug, Clone)]
pub struct BentoNer {
    /// Pre-built endpoint at the `/recognize` route. Cloned per
    /// call so per-request headers (`x-request-id`) layer onto a
    /// fresh instance without rebuilding the route.
    endpoint: Endpoint,
    /// Service-side model identifier echoed in provenance.
    model_id: HipStr<'static>,
    /// Default per-label confidence cutoff sent on every request.
    /// Per-label thresholds in the schema override it.
    default_threshold: f32,
}

impl BentoNer {
    /// Build from a service URL + the deployment's model id. The
    /// default per-label threshold starts at `0.5` (matches the
    /// service's own default); use [`with_default_threshold`] to
    /// override.
    ///
    /// [`with_default_threshold`]: Self::with_default_threshold
    pub fn new(
        base_url: impl Into<String>,
        model_id: impl Into<HipStr<'static>>,
    ) -> Result<Self> {
        let client = Client::builder()
            .with_base_url(base_url)
            .build()
            .map_err(BentoError::Transport)?;
        Ok(Self {
            endpoint: client.endpoint(ROUTE),
            model_id: model_id.into(),
            default_threshold: 0.5,
        })
    }

    /// Override the per-request default confidence threshold (the
    /// service applies it when a schema entity has no per-label
    /// `threshold` of its own).
    #[must_use]
    pub fn with_default_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold;
        self
    }

    /// Send one batched `/recognize` POST and parse the response
    /// body. Clones the cached endpoint so per-request headers
    /// layer on without touching the original.
    async fn post_recognize(
        &self,
        requests: &[NerRequest<'_>],
    ) -> Result<Vec<WireNerResponse>, BentoError> {
        let body: Vec<WireNerRequest> = requests
            .iter()
            .map(|r| WireNerRequest::from_request(r, self.default_threshold))
            .collect();
        let mut endpoint = self.endpoint.clone();
        if let Some(id) = requests.iter().find_map(|r| r.correlation_id) {
            endpoint = endpoint.with_request_id(id.to_string());
        }
        endpoint
            .invoke::<_, Vec<WireNerResponse>>(&body)
            .await
            .map_err(BentoError::Transport)
    }
}

#[async_trait::async_trait]
impl NerBackend for BentoNer {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn recognize(&self, request: NerRequest<'_>) -> Result<NerResponse> {
        let responses = self.post_recognize(&[request]).await?;
        let mut iter = responses.into_iter();
        let response = iter.next().ok_or_else(|| {
            BentoError::Protocol("bento ner returned an empty batch".into())
        })?;
        if iter.next().is_some() {
            return Err(BentoError::Protocol(
                "bento ner returned more responses than requests".into(),
            )
            .into());
        }
        Ok(response.decode())
    }

    async fn recognize_batch(&self, requests: &[NerRequest<'_>]) -> Result<Vec<NerResponse>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let responses = self.post_recognize(requests).await?;
        if responses.len() != requests.len() {
            return Err(BentoError::Protocol(format!(
                "bento ner returned {} responses for {} requests",
                responses.len(),
                requests.len(),
            ))
            .into());
        }
        Ok(responses.into_iter().map(WireNerResponse::decode).collect())
    }
}
