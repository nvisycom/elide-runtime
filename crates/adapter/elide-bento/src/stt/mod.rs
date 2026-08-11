//! [`BentoStt`]: an [`SttBackend`] backed by the
//! `nvisy-inference-stt` BentoML service.
//!
//! Wire contract: `POST /transcribe` accepts a single request
//! (base64-encoded audio bytes plus optional filename and
//! language hint) and returns a response whose `segments` list
//! carries per-segment timings (milliseconds), text, and
//! optional per-word breakdowns. This backend flattens the
//! response onto elide's [`TranscriptSegment`]/[`TranscriptWord`]
//! vocabulary one-to-one. Per-call correlation IDs propagate as
//! `x-request-id` headers when set.
//!
//! Wire types live in the private `request` (outgoing) and
//! `response` (incoming) submodules; only the public
//! [`BentoStt`] backend is part of this crate's API.
//!
//! [`SttBackend`]: elide_stt::SttBackend
//! [`TranscriptSegment`]: elide_core::modality::audio::TranscriptSegment
//! [`TranscriptWord`]: elide_core::modality::audio::TranscriptWord

mod request;
mod response;

use bentoml::{Client, Endpoint};
use elide_core::Result;
use elide_core::entity::provenance::ModelEvent;
use elide_stt::{SttBackend, SttRequest, SttResponse};
use hipstr::HipStr;

use self::request::WireSttRequest;
use self::response::WireSttResponse;
use crate::error::BentoError;

const ROUTE: &str = "transcribe";

/// BentoML STT backend.
///
/// Owns a cached [`Endpoint`] pointing at the `nvisy-inference-stt`
/// `/transcribe` route, plus the per-deployment model id (echoed
/// into [`SttBackend::provenance`]).
#[derive(Debug, Clone)]
pub struct BentoStt {
    /// Pre-built endpoint at the `/transcribe` route. Cloned per
    /// call so per-request headers (`x-request-id`) layer onto a
    /// fresh instance without rebuilding the route.
    endpoint: Endpoint,
    /// Service-side model identifier echoed in provenance.
    model_id: HipStr<'static>,
}

impl BentoStt {
    /// Build from a service URL + the deployment's model id.
    pub fn new(base_url: impl Into<String>, model_id: impl Into<HipStr<'static>>) -> Result<Self> {
        let client = Client::builder()
            .with_base_url(base_url)
            .build()
            .map_err(BentoError::Transport)?;
        Ok(Self {
            endpoint: client.endpoint(ROUTE),
            model_id: model_id.into(),
        })
    }

    /// Send one `/transcribe` POST end-to-end: encode the
    /// [`SttRequest`] to its wire form, POST it (layering
    /// `x-request-id` when the request carries a correlation id),
    /// decode the wire response back to an [`SttResponse`].
    async fn post_transcribe(&self, request: SttRequest<'_>) -> Result<SttResponse, BentoError> {
        let body = WireSttRequest::from_request(&request);
        let mut endpoint = self.endpoint.clone();
        if let Some(id) = request.correlation_id {
            endpoint = endpoint.with_request_id(id.to_string());
        }
        let wire: WireSttResponse = endpoint
            .invoke(&body)
            .await
            .map_err(BentoError::Transport)?;
        Ok(wire.decode())
    }
}

#[async_trait::async_trait]
impl SttBackend for BentoStt {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn transcribe(&self, request: SttRequest<'_>) -> Result<SttResponse> {
        Ok(self.post_transcribe(request).await?)
    }
}
