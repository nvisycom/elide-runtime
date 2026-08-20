//! Incoming wire types for the NER `/recognize` endpoint.
//!
//! Mirrors `elide_bento_core.ner.v1.NerResponse` from the inference
//! repository. Classifications, structures, and the response-level
//! `modelId` are deserialised-and-discarded — this backend surfaces
//! entity-extraction results only.
//!
//! The `tokens` the service reports are always parsed, but only carried
//! onto the response under the `usage` feature.

#[cfg(feature = "usage")]
use elide_core::recognition::TokenCounts;
use elide_ner::backend::{NerResponse, NerSpan};
use serde::Deserialize;

/// Incoming per-call response body element.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireNerResponse {
    /// Extracted entities, in backend order.
    #[serde(default)]
    pub entities: Vec<WireEntity>,
    /// Encoder tokens the call spent. Absent from a service that predates
    /// the field, so always optional. Read only under `usage`, but always
    /// parsed so the response deserializes either way.
    #[serde(default)]
    #[cfg_attr(
        not(feature = "usage"),
        allow(dead_code, reason = "read only under `usage`")
    )]
    pub tokens: Option<WireTokenUsage>,
    // `classifications`, `structures`, `modelId` ignored.
}

/// Encoder token usage for one call.
///
/// GLiNER2 is an encoder: it scores spans over the input and generates
/// nothing, so there is an input count but no output count. `limit` is the
/// model's per-input maximum, carried so a consumer can see headroom.
// Parsed whatever the feature set: the fields are read only under `usage`,
// but the type must still deserialize so a response carrying `tokens` does
// not fail to parse in a default build.
#[cfg_attr(
    not(feature = "usage"),
    allow(dead_code, reason = "read only under `usage`")
)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireTokenUsage {
    /// Encoder tokens in the request text.
    pub input: u64,
    /// The model's token limit for one input. Parsed for contract fidelity;
    /// elide's `TokenCounts` has no field for a capacity, so it is not
    /// surfaced yet.
    #[allow(dead_code, reason = "no TokenCounts field for capacity yet")]
    pub limit: u64,
}

/// One extracted entity span.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireEntity {
    /// Model-native label string.
    pub label: String,
    /// Confidence in `[0, 1]`.
    pub score: f32,
    /// Byte offset, inclusive.
    pub start: usize,
    /// Byte offset, exclusive.
    pub end: usize,
}

impl WireNerResponse {
    /// Translate into the elide [`NerResponse`] the backend trait
    /// expects. Drops malformed (`end <= start`) spans defensively
    /// — the wire validator already rejects them, but the guard
    /// keeps a misbehaving service from poisoning the recognizer.
    pub(super) fn decode(self) -> NerResponse {
        #[cfg(feature = "usage")]
        let tokens = self.tokens.as_ref().map(|t| TokenCounts {
            // Input only: an encoder generates nothing, so `output` stays
            // `None` and the total is the input count.
            input: Some(t.input),
            output: None,
            total: Some(t.input),
        });
        let spans = self
            .entities
            .into_iter()
            .filter_map(|e| {
                if e.end <= e.start {
                    return None;
                }
                Some(NerSpan::new(e.label, e.score, e.start..e.end))
            })
            .collect();
        let response = NerResponse::new(spans);
        #[cfg(feature = "usage")]
        let response = match tokens {
            Some(t) => response.with_tokens(t),
            None => response,
        };
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response from a service that reports `tokens` parses, and the
    /// malformed-span guard still drops `end <= start`.
    #[test]
    fn decodes_tokens_and_drops_malformed_spans() {
        let json = r#"{
            "entities": [
                {"label": "person", "score": 0.9, "start": 0, "end": 5},
                {"label": "email", "score": 0.8, "start": 7, "end": 7}
            ],
            "tokens": {"input": 12, "limit": 512},
            "modelId": "fastino/gliner2-privacy-filter-PII-multi"
        }"#;
        let wire: WireNerResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            wire.tokens.as_ref().map(|t| (t.input, t.limit)),
            Some((12, 512))
        );

        let decoded = wire.decode();
        // The zero-width span is dropped; the valid one survives.
        assert_eq!(decoded.spans.len(), 1);
        #[cfg(feature = "usage")]
        {
            assert_eq!(decoded.tokens.input, Some(12));
            // An encoder generates nothing, so output is absent and the
            // total is the input count.
            assert_eq!(decoded.tokens.output, None);
            assert_eq!(decoded.tokens.total, Some(12));
        }
    }

    /// A service that predates the field omits `tokens` entirely; the
    /// response must still parse, with no usage attached.
    #[test]
    fn decodes_response_without_tokens() {
        let json = r#"{"entities": [], "modelId": "m"}"#;
        let wire: WireNerResponse = serde_json::from_str(json).unwrap();
        assert!(wire.tokens.is_none());

        let decoded = wire.decode();
        assert!(decoded.spans.is_empty());
        #[cfg(feature = "usage")]
        assert!(decoded.tokens.is_empty());
    }
}
