//! [`BentoNer`]: a [`Backend`] backed by the
//! `bento-gliner2` BentoML service.
//!
//! Wire contract: `POST /recognize` accepts a batched list of
//! requests (one schema-driven entity-extraction call per item)
//! and returns the matching list of responses. Each request carries
//! a schema (entities + optional classifications + structures); this
//! backend uses entities only and ignores the rest. Per-call
//! correlation IDs propagate as `x-request-id` headers when set.
//!
//! Wire types live in the private `request` (outgoing) and
//! `response` (incoming) submodules; only the public
//! [`BentoNer`] backend is part of this crate's API.
//!
//! [`Backend`]: elide_core::backend::Backend

mod request;
mod response;

use bentoml::{Client, Endpoint};
use elide_core::Result;
use elide_core::backend::Backend;
use elide_core::entity::audit::ModelEvent;
use elide_ner::backend::{NerRequest, NerResponse};
use hipstr::HipStr;

use self::request::WireNerRequest;
use self::response::WireNerResponse;
use crate::error::BentoError;

const ROUTE: &str = "recognize";

/// BentoML NER backend.
///
/// Owns a cached [`Endpoint`] pointing at the `bento-gliner2`
/// `/recognize` route, plus the per-deployment model id (echoed
/// into [`Backend::provenance`]) and a default per-label
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
    pub fn new(base_url: impl Into<String>, model_id: impl Into<HipStr<'static>>) -> Result<Self> {
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

    /// Send one batched `/recognize` POST end-to-end: encode
    /// each [`NerRequest`] to its wire form, POST the batch
    /// (layering `x-request-id` when any request carries a
    /// correlation id), decode the wire responses back to
    /// [`NerResponse`]s.
    async fn post_recognize(
        &self,
        requests: &[NerRequest<'_>],
    ) -> Result<Vec<NerResponse>, BentoError> {
        let body: Vec<WireNerRequest> = requests
            .iter()
            .map(|r| WireNerRequest::from_request(r, self.default_threshold))
            .collect();
        let mut endpoint = self.endpoint.clone();
        if let Some(id) = requests.iter().find_map(|r| r.correlation_id) {
            endpoint = endpoint.with_request_id(id.to_string());
        }
        let wire: Vec<WireNerResponse> = endpoint
            .invoke(&body)
            .await
            .map_err(BentoError::Transport)?;
        Ok(wire.into_iter().map(WireNerResponse::decode).collect())
    }
}

#[async_trait::async_trait]
impl Backend for BentoNer {
    type Request<'a> = NerRequest<'a>;
    type Response = NerResponse;

    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn call(&self, request: NerRequest<'_>) -> Result<NerResponse> {
        // One request in, so exactly one response out. `pop` alone would
        // take the last of a longer list and hide the contract violation.
        let responses = self.post_recognize(&[request]).await?;
        let [response] = <[NerResponse; 1]>::try_from(responses).map_err(|responses| {
            BentoError::Protocol(format!(
                "bento ner returned {} responses for 1 request",
                responses.len(),
            ))
        })?;
        Ok(response)
    }

    /// Overridden: the `bento-gliner2` service takes a batch in one POST, so the
    /// whole slice goes in a single round trip rather than the default's
    /// sequential fan-out over [`call`](Self::call).
    ///
    /// Responses come back in request order. An empty batch makes no call.
    async fn call_batch(&self, requests: Vec<NerRequest<'_>>) -> Result<Vec<NerResponse>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let responses = self.post_recognize(&requests).await?;
        if responses.len() != requests.len() {
            return Err(BentoError::Protocol(format!(
                "bento ner returned {} responses for {} requests",
                responses.len(),
                requests.len(),
            ))
            .into());
        }
        Ok(responses)
    }
}
