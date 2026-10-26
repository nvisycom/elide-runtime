//! [`BentoStt`]: a [`Backend`] backed by the
//! `elide-stt` BentoML service.
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
//! [`Backend`]: elide_core::backend::Backend
//! [`TranscriptSegment`]: elide_audio::modality::TranscriptSegment
//! [`TranscriptWord`]: elide_audio::modality::TranscriptWord

mod request;
mod response;

use std::time::Duration;

use bentoml::{Client, Endpoint};
use elide_audio::stt::{SttRequest, SttResponse};
use elide_core::Result;
use elide_core::backend::Backend;
use elide_core::entity::audit::ModelEvent;
use hipstr::HipStr;
use uuid::Uuid;

use self::request::{Transcribe, WireSttRequest};
use self::response::WireSttResponse;
use crate::error::BentoError;

const ROUTE: &str = "transcribe";

/// BentoML STT backend.
///
/// Owns a cached [`Endpoint`] pointing at the `elide-stt`
/// `/transcribe` route, plus the per-deployment model id (echoed
/// into [`Backend::provenance`]).
#[derive(Debug, Clone)]
pub struct BentoStt {
    /// Pre-built endpoint at the `/transcribe` route. Cloned per
    /// call so per-request headers (`x-request-id`) layer onto a
    /// fresh instance without rebuilding the route.
    /// Kept past construction so readiness can be probed: `/readyz` is a
    /// service-level endpoint, not a route, and `Endpoint` does not expose the
    /// client it was built from.
    client: Client,
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
            client,
            model_id: model_id.into(),
        })
    }

    /// Wait for the service to report itself ready, or give up after
    /// `timeout`.
    ///
    /// Construction does not connect, so the first call to a service that is
    /// still loading its model fails as a transport error — and the window is
    /// not small: these services bind the port before the model is loaded, and
    /// a restarted container spends ~9s reloading it. A caller that starts
    /// alongside the service should wait here rather than discover this one
    /// failed request at a time.
    ///
    /// Not called from [`new`](Self::new): construction stays non-async and
    /// free, so a caller that already knows the service is up pays nothing,
    /// and one that does not chooses its own deadline.
    pub async fn wait_until_ready(&self, timeout: Duration, interval: Duration) -> Result<()> {
        self.client
            .wait_until_ready(timeout, interval)
            .await
            .map_err(BentoError::Transport)?;
        Ok(())
    }

    /// Send one `/transcribe` POST end-to-end: encode the
    /// [`SttRequest`] to its wire form, POST it (layering
    /// `x-request-id` when the request carries a correlation id),
    /// decode the wire response back to an [`SttResponse`].
    async fn post_transcribe(&self, request: SttRequest<'_>) -> Result<SttResponse, BentoError> {
        let body = WireSttRequest::from_request(&request);
        // Always sent, generated when the caller has none; see the note in the
        // NER adapter for why a missing id is the case that matters.
        let request_id = request.correlation_id.unwrap_or_else(Uuid::new_v4);
        let endpoint = self
            .endpoint
            .clone()
            .with_request_id(request_id.to_string());
        let wire: WireSttResponse = endpoint
            .invoke(&Transcribe { request: &body })
            .await
            .map_err(BentoError::Transport)?;
        Ok(wire.decode())
    }
}

#[async_trait::async_trait]
impl Backend for BentoStt {
    type Request<'a> = SttRequest<'a>;
    type Response = SttResponse;

    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: self.model_id.clone(),
            version: None,
            contextual: false,
        }
    }

    async fn call(&self, request: SttRequest<'_>) -> Result<SttResponse> {
        Ok(self.post_transcribe(request).await?)
    }
}
