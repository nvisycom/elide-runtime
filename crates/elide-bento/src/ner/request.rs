//! Outgoing wire types for the NER `/recognize` endpoint.
//!
//! Mirrors `nvisy_core.ner.v1.NerRequest` from the inference
//! repository: a `(text, schema, threshold)` triple where the
//! schema lists the entities to extract. Classifications and
//! structured records (also part of the upstream schema) are
//! omitted — this backend surfaces entity extraction only.

use elide_core::primitive::LanguageTag;
use elide_ner::backend::NerRequest;
use serde::Serialize;

/// Outgoing per-call request body element.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireNerRequest {
    /// Source text to scan.
    pub text: String,
    /// Schema describing what to extract.
    pub schema: WireSchema,
    /// Default per-label confidence cutoff. Per-entity `threshold`
    /// overrides this when present.
    pub threshold: f32,
}

impl WireNerRequest {
    /// Translate an elide [`NerRequest`] into the wire shape,
    /// pinning the service-default threshold when the request has
    /// no per-label thresholds of its own.
    ///
    /// Label name/description are localized in the request's
    /// asserted language when set; otherwise English is used as
    /// the fallback locale (matching elide's own default).
    pub(super) fn from_request(request: &NerRequest<'_>, default_threshold: f32) -> Self {
        let english = LanguageTag::english();
        let language = request.language.unwrap_or(&english);
        let entities = request
            .labels
            .map(|labels| {
                labels
                    .iter()
                    .map(|label| WireEntitySpec {
                        label: label.name(language).to_owned(),
                        description: label.description(language).map(str::to_owned),
                        threshold: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            text: request.text.to_owned(),
            schema: WireSchema { entities },
            threshold: default_threshold,
        }
    }
}

/// `Schema` group: the entities the call extracts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireSchema {
    pub entities: Vec<WireEntitySpec>,
}

/// One entity to extract.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireEntitySpec {
    /// Stable label identifier (e.g. `"email_address"`).
    pub label: String,
    /// Optional natural-language description that steers zero-shot
    /// extraction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Per-label confidence cutoff. `None` falls through to the
    /// request-level [`WireNerRequest::threshold`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
}
