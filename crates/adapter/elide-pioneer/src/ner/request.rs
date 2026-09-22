//! Outgoing wire types for `POST /gliner-2`.
//!
//! Mirrors the body the `gliner2` Python client sends. Pioneer's own
//! enrichment tasks (classification, structured records, relations) are
//! not modelled: elide's [`NerBackend`] asks for entity spans, and the
//! extra results would be discarded on the way out.
//!
//! [`NerBackend`]: elide_ner::backend::NerBackend

use serde::Serialize;

/// The task this backend asks for. Pioneer routes on it.
const TASK: &str = "extract_entities";

/// One extraction request.
#[derive(Debug, Serialize)]
pub(super) struct WireRequest<'a> {
    pub task: &'static str,
    pub text: &'a str,
    /// The labels to extract. Bare names: Pioneer's wire schema is a list
    /// of strings, and the descriptions elide's `Label` can carry have no
    /// slot in it.
    pub schema: Vec<String>,
    pub threshold: f32,
    /// Both always on: without them the response carries no confidence and
    /// no offsets, which are exactly what a `NerSpan` is made of.
    pub include_confidence: bool,
    pub include_spans: bool,
    /// Keeps the label-keyed shape this crate decodes.
    pub format_results: bool,
    /// Ask Pioneer not to persist this request or its response.
    ///
    /// Omitted rather than sent as `true`, so a deployment that has not
    /// opted in gets Pioneer's default rather than a silently different
    /// one. Their documentation scopes zero-retention to "eligible use
    /// cases" without enumerating them, so this is a request, not a
    /// guarantee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
}

impl<'a> WireRequest<'a> {
    /// A request over `text` for `labels`.
    pub(super) fn new(
        text: &'a str,
        labels: Vec<String>,
        threshold: f32,
        zero_retention: bool,
    ) -> Self {
        Self {
            task: TASK,
            text,
            schema: labels,
            threshold,
            include_confidence: true,
            include_spans: true,
            format_results: true,
            store: zero_retention.then_some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `store` is omitted unless asked for, so a deployment that has not
    /// opted in gets Pioneer's default rather than a silently different
    /// one.
    #[test]
    fn store_is_omitted_by_default() {
        let body = serde_json::to_string(&WireRequest::new("x", vec![], 0.5, false)).unwrap();
        assert!(!body.contains("store"));
    }

    /// Opting in sends `store: false` — "do not persist", not "do store".
    #[test]
    fn zero_retention_sends_store_false() {
        let body = serde_json::to_string(&WireRequest::new("x", vec![], 0.5, true)).unwrap();
        assert!(body.contains(r#""store":false"#));
    }

    /// The flags a `NerSpan` depends on are always on: without them the
    /// response carries neither a score nor an offset.
    #[test]
    fn confidence_and_spans_are_always_requested() {
        let body = serde_json::to_string(&WireRequest::new("x", vec![], 0.5, false)).unwrap();
        assert!(body.contains(r#""include_confidence":true"#));
        assert!(body.contains(r#""include_spans":true"#));
    }
}
