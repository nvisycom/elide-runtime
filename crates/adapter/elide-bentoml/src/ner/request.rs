//! Outgoing wire types for the NER `/recognize` endpoint.
//!
//! Mirrors `elide_core.ner.v1.NerRequest` from the inference
//! repository: a `(text, schema, threshold)` triple where the
//! schema lists the entities to extract. Classifications and
//! structured records (also part of the upstream schema) are
//! omitted — this backend surfaces entity extraction only.

use elide_core::primitive::LanguageTag;
use elide_ner::backend::NerRequest;
use serde::Serialize;

/// The `/recognize` input model.
///
/// BentoML keys an endpoint's input on its handler's parameter names, so
/// `recognize(self, requests: list[NerRequest], ctx)` takes
/// `{"requests": [...]}` and rejects a bare array with
/// `400 1 validation error for Input`. The service publishes the same shape
/// in its OpenAPI document, where `/recognize` reports `required: ["requests"]`.
#[derive(Debug, Serialize)]
pub(super) struct Recognize<'a> {
    /// Named for the handler parameter that receives it.
    pub requests: &'a [WireNerRequest],
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The body must be the object the service's OpenAPI declares, not the
    /// bare array this client posted until `bentoml` 0.6. Nothing else checks
    /// this: the adapter's own transport is mocked, so a wrong shape reaches
    /// only a real service — which answered `400 1 validation error for
    /// Input` for every call.
    #[test]
    fn the_body_is_an_object_keyed_on_the_handler_parameter() {
        let requests = vec![WireNerRequest {
            text: "Ana".to_owned(),
            schema: WireSchema {
                entities: Vec::new(),
            },
            threshold: 0.3,
        }];

        let json = serde_json::to_value(Recognize {
            requests: &requests,
        })
        .expect("the request serializes");

        assert!(
            json.get("requests").is_some_and(|r| r.is_array()),
            "`/recognize` declares `required: [\"requests\"]`, got {json}"
        );
    }
}
