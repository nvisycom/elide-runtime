//! Incoming wire types for the NER `/recognize` endpoint.
//!
//! Mirrors `nvisy_core.ner.v1.NerResponse` from the inference
//! repository. Classifications, structures, and the response-level
//! `modelId` are deserialised-and-discarded — this backend surfaces
//! entity-extraction results only.

use elide_ner::backend::{NerResponse, NerSpan};
use serde::Deserialize;

/// Incoming per-call response body element.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireNerResponse {
    /// Extracted entities, in backend order.
    #[serde(default)]
    pub entities: Vec<WireEntity>,
    // `classifications`, `structures`, `modelId` ignored.
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
        NerResponse::new(spans)
    }
}
