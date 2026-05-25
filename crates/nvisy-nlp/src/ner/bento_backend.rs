//! [`BentoNerBackend`] — zero-shot NER over an externalized HTTP
//! inference service (the `inference-gliner` Bento in
//! [`nvisycom/inference`]).
//!
//! The wire shape mirrors `nvisy_core.ner.v1` from the inference
//! repo: the request batches one or more text-and-kinds tuples,
//! and the response carries spans already classified into the
//! canonical [`EntityKind`] taxonomy. The service owns the
//! label-map translation, so swapping the underlying model never
//! changes this code.
//!
//! Per-request `correlation_id` propagation rides on the
//! `x-request-id` header via [`Endpoint::with_request_id`]; the
//! Python service reads it from `ctx.request_id` and threads it
//! through its logs.
//!
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference
//! [`Endpoint::with_request_id`]: bentoml::Endpoint::with_request_id

use async_trait::async_trait;
use bentoml::prelude::*;
use nvisy_ontology::entity::Entities;
use uuid::Uuid;

use super::bento_types::{WireBatch, WireRequest, WireResponse};
use super::{NerBackend, NerParams};
use crate::error::{Error, Result};

const TARGET: &str = "nvisy_nlp::ner::bento";

/// The route exposed by the `inference-gliner` Bento.
const RECOGNIZE_ROUTE: &str = "recognize";

/// Parameters for [`BentoNerBackend`].
#[derive(Debug, Clone)]
pub struct BentoNerParams {
    /// Base URL of the `inference-gliner` Bento (e.g. `http://localhost:3000`).
    pub base_url: String,
    /// Optional per-call `correlation_id` used as the `x-request-id`
    /// header value. When `None`, the backend generates a UUIDv7
    /// per call so every request is traceable.
    pub correlation_id: Option<Uuid>,
}

impl BentoNerParams {
    /// Construct with the given service URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            correlation_id: None,
        }
    }
}

/// [`NerBackend`] that calls an externalized GLiNER-backed BentoML
/// service over HTTP.
#[derive(Debug)]
pub struct BentoNerBackend {
    client: Client,
    correlation_id: Option<Uuid>,
}

impl BentoNerBackend {
    /// Build a backend against the given parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be
    /// constructed (invalid `base_url`).
    pub fn new(params: BentoNerParams) -> Result<Self> {
        let client = Client::builder()
            .with_base_url(&params.base_url)
            .build()
            .map_err(|e| Error::Backend(format!("bentoml client init: {e}")))?;
        Ok(Self {
            client,
            correlation_id: params.correlation_id,
        })
    }
}

#[async_trait]
impl NerBackend for BentoNerBackend {
    async fn recognize(&self, text: &str, params: NerParams<'_>) -> Result<Entities> {
        let mut out = self.recognize_batch(&[text], params).await?;
        Ok(out.pop().unwrap_or_default())
    }

    #[tracing::instrument(skip_all, fields(batch = texts.len()))]
    async fn recognize_batch(
        &self,
        texts: &[&str],
        params: NerParams<'_>,
    ) -> Result<Vec<Entities>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // GLiNER is zero-shot — without a kinds list the service has
        // nothing to look for. Short-circuit locally.
        let Some(kinds) = params.requested_kinds.filter(|k| !k.is_empty()) else {
            return Ok(vec![Entities::new(); texts.len()]);
        };

        let language = params.language.map(|l| l.as_str().to_owned());
        let wire_requests = texts
            .iter()
            .map(|text| WireRequest {
                text: (*text).to_owned(),
                kinds: kinds.to_vec(),
                threshold: 0.0,
                language: language.clone(),
            })
            .collect();

        let request_id = self
            .correlation_id
            .unwrap_or_else(Uuid::now_v7)
            .to_string();

        let responses: Vec<WireResponse> = self
            .client
            .endpoint(RECOGNIZE_ROUTE)
            .with_request_id(&request_id)
            .call(&WireBatch { requests: wire_requests })
            .await
            .map_err(|e| Error::Backend(format!("bento ner call: {e}")))?;

        if responses.len() != texts.len() {
            tracing::warn!(
                target: TARGET,
                requested = texts.len(),
                received = responses.len(),
                "bento ner response length does not match batch request",
            );
        }

        let mut out = Vec::with_capacity(texts.len());
        for i in 0..texts.len() {
            match responses.get(i) {
                Some(response) => out.push(Entities(
                    response
                        .entities
                        .iter()
                        .map(|e| e.to_entity(&response.model))
                        .collect(),
                )),
                None => out.push(Entities::new()),
            }
        }
        Ok(out)
    }
}
