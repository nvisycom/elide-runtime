//! [`BentoBackend`]: zero-shot NER over the externalised
//! `inference-gliner` Bento.
//!
//! Implements [`GlinerBackend`]. The wire shape mirrors
//! `nvisy_core.ner.v1` from [`nvisycom/inference`]; per-request
//! `correlation_id` propagation rides on the `x-request-id`
//! header.
//!
//! Wire compatibility note: the inference service today returns
//! `kind: EntityKind` (already normalized server-side); this
//! backend serialises that back to a string label so the
//! [`GlinerRecognizer`]'s
//! [`LabelMap`] sees a uniform raw
//! label regardless of which backend produced it. When the
//! service later moves to returning raw model labels, only the
//! wire-type module needs to change.
//!
//! [`nvisycom/inference`]: https://github.com/nvisycom/inference
//! [`GlinerRecognizer`]: crate::recognition::GlinerRecognizer
//! [`LabelMap`]: crate::recognition::LabelMap

use bentoml::prelude::*;
use nvisy_core::nlp::RawNerSpan;
use nvisy_core::{Error, Result};
use uuid::Uuid;

use super::bento_types::{WireBatch, WireRequest, WireResponse};
use super::gliner_backend::{GlinerBackend, GlinerRequest};

const COMPONENT: &str = "ner-bento";
const RECOGNIZE_ROUTE: &str = "recognize";

/// Construction parameters for [`BentoBackend`].
#[derive(Debug, Clone)]
pub struct BentoParams {
    /// Base URL of the `inference-gliner` Bento (e.g.
    /// `http://localhost:3000`).
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

/// [`GlinerBackend`] backed by an externalised BentoML service.
#[derive(Debug)]
pub struct BentoBackend {
    endpoint: Endpoint,
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
        Ok(Self {
            endpoint: client.endpoint(RECOGNIZE_ROUTE),
        })
    }
}

#[async_trait::async_trait]
impl GlinerBackend for BentoBackend {
    #[tracing::instrument(skip_all, fields(kinds = request.kinds.len()))]
    async fn predict(&self, request: GlinerRequest<'_>) -> Result<Vec<RawNerSpan>> {
        if request.kinds.is_empty() {
            return Ok(Vec::new());
        }

        let language = request.language.map(|l| l.as_str().to_owned());
        let wire_request = WireRequest {
            text: request.text.to_owned(),
            kinds: request.kinds.to_vec(),
            threshold: 0.0,
            language,
        };
        let request_id = request
            .correlation_id
            .unwrap_or_else(Uuid::now_v7)
            .to_string();

        let responses: Vec<WireResponse> = self
            .endpoint
            .clone()
            .with_request_id(&request_id)
            .invoke(&WireBatch {
                requests: vec![wire_request],
            })
            .await
            .map_err(|e| Error::runtime(format!("bento ner call: {e}"), COMPONENT, true))?;

        let mut spans = Vec::new();
        for response in &responses {
            for entity in &response.entities {
                spans.push(RawNerSpan::new(
                    entity_kind_label(entity.kind),
                    entity.score,
                    entity.start..entity.end,
                ));
            }
        }
        Ok(spans)
    }
}

/// Serialise an [`EntityKind`] to its canonical snake_case label.
/// The Bento service already returns normalized kinds; we convert
/// back to the wire label so [`LabelMap`]
/// sees a uniform raw label across backends.
///
/// [`LabelMap`]: crate::recognition::LabelMap
fn entity_kind_label(kind: nvisy_ontology::entity::EntityKind) -> String {
    kind.to_string()
}
