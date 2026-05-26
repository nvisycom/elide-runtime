//! [`BentoBackend`] — zero-shot NER over an externalized HTTP
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
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;
use uuid::Uuid;

use super::bento_types::{WireBatch, WireRequest, WireResponse};
use crate::core::{Backend, Context};

const TARGET: &str = "nvisy_ner::backend::bento";
const COMPONENT: &str = "ner-bento";

/// The route exposed by the `inference-gliner` Bento.
const RECOGNIZE_ROUTE: &str = "recognize";

/// Parameters for [`BentoBackend`].
#[derive(Debug, Clone)]
pub struct BentoParams {
    /// Base URL of the `inference-gliner` Bento (e.g. `http://localhost:3000`).
    pub base_url: String,
}

impl BentoParams {
    /// Construct with the given service URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

/// [`Backend`] that calls an externalized GLiNER-backed BentoML
/// service over HTTP.
#[derive(Debug)]
pub struct BentoBackend {
    client: Client,
}

impl BentoBackend {
    /// Build a backend against the given parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying HTTP client cannot be
    /// constructed (invalid `base_url`).
    pub fn new(params: BentoParams) -> Result<Self> {
        let client = Client::builder()
            .with_base_url(&params.base_url)
            .build()
            .map_err(|e| Error::runtime(format!("bentoml client init: {e}"), COMPONENT, false))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Backend for BentoBackend {
    async fn recognize(&self, text: &str, ctx: &Context) -> Result<Vec<Entity<Text>>> {
        self.recognize_batch(&[text], ctx).await
    }

    #[tracing::instrument(skip_all, fields(batch = texts.len()))]
    async fn recognize_batch(&self, texts: &[&str], ctx: &Context) -> Result<Vec<Entity<Text>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // GLiNER is zero-shot — without an entities allowlist the
        // service has nothing to look for. Short-circuit locally.
        let Some(kinds) = ctx.entity_kinds.as_deref().filter(|k| !k.is_empty()) else {
            return Ok(Vec::new());
        };

        let language = ctx.language.as_ref().map(|l| l.as_str().to_owned());
        let wire_requests = texts
            .iter()
            .map(|text| WireRequest {
                text: (*text).to_owned(),
                kinds: kinds.to_vec(),
                threshold: 0.0,
                language: language.clone(),
            })
            .collect();

        let request_id = ctx.correlation_id.unwrap_or_else(Uuid::now_v7).to_string();

        let responses: Vec<WireResponse> = self
            .client
            .endpoint(RECOGNIZE_ROUTE)
            .with_request_id(&request_id)
            .call(&WireBatch {
                requests: wire_requests,
            })
            .await
            .map_err(|e| Error::runtime(format!("bento ner call: {e}"), COMPONENT, true))?;

        if responses.len() != texts.len() {
            tracing::warn!(
                target: TARGET,
                requested = texts.len(),
                received = responses.len(),
                "bento ner response length does not match batch request",
            );
        }

        // Merge per-text entities into one Entities — texts are
        // assumed to be chunks of the same source (see Backend
        // trait docs).
        let mut merged: Vec<Entity<Text>> = Vec::new();
        for response in &responses {
            merged.extend(
                response
                    .entities
                    .iter()
                    .map(|e| e.to_entity(&response.model)),
            );
        }
        Ok(merged)
    }
}
