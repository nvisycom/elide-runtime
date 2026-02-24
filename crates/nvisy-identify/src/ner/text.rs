//! AI-powered named-entity recognition (NER) detection layer for text.
//!
//! Uses a [`SequentialContext`] so the orchestrator feeds one span at
//! a time, allowing the layer to accumulate prior text/entities
//! between spans via interior mutability.

use serde::Deserialize;
use tokio::sync::Mutex;

use nvisy_codec::handler::{Span, TxtSpan};
use nvisy_ontology::entity::EntityKind;
use nvisy_core::Error;

use super::{NerBackend, NerConfig, parse_ner_entities};
use crate::{Entity, Location, ModelInfo, TextLocation};
use crate::{SequentialContext, DetectionService};

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`NerDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NerDetectionParams {
    /// Entity kinds to detect (empty = all).
    #[serde(rename = "entityTypes", default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
    /// Optional model info to attach to every NER-produced entity.
    #[serde(skip)]
    pub model_info: Option<ModelInfo>,
}

/// Accumulated state between sequential span calls.
struct NerState {
    /// Text from previously processed spans (for sliding context).
    prior_text: String,
}

/// AI NER detection layer — delegates to an [`NerBackend`] at runtime.
///
/// Uses [`SequentialContext`]: the orchestrator feeds one span at a
/// time so the layer can carry sliding context between spans.
pub struct NerDetection<B> {
    backend: B,
    config: NerConfig,
    model_info: Option<ModelInfo>,
    state: Mutex<NerState>,
}

impl<B: NerBackend> NerDetection<B> {
    /// Create a new detection layer with the given backend and params.
    pub fn new(backend: B, params: NerDetectionParams) -> Self {
        let config = NerConfig {
            entity_types: params.entity_kinds.iter().map(|ek| ek.to_string()).collect(),
            confidence_threshold: params.confidence_threshold,
        };
        Self {
            backend,
            config,
            model_info: params.model_info,
            state: Mutex::new(NerState {
                prior_text: String::new(),
            }),
        }
    }

    /// Clear accumulated state between documents.
    pub async fn reset(&self) {
        let mut state = self.state.lock().await;
        state.prior_text.clear();
    }
}

#[async_trait::async_trait]
impl<B: NerBackend> DetectionService<TxtSpan, String> for NerDetection<B> {
    type Context = SequentialContext;

    async fn detect(
        &self,
        spans: Vec<Span<TxtSpan, String>>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            // Build the full text with prior context prepended.
            let (full_text, context_len) = {
                let state = self.state.lock().await;
                if state.prior_text.is_empty() {
                    (span.data.clone(), 0)
                } else {
                    let sep = "\n";
                    let context_len = state.prior_text.len() + sep.len();
                    let full = format!("{}{}{}", state.prior_text, sep, span.data);
                    (full, context_len)
                }
            };

            let raw = self
                .backend
                .detect_text(&full_text, &self.config)
                .await?;

            // Filter entities to the current span and adjust offsets.
            let span_len = span.data.len();
            for mut e in parse_ner_entities(&raw)? {
                if let Some(Location::Text(ref loc)) = e.location {
                    // Skip entities that fall entirely within the prior context.
                    if loc.end_offset <= context_len {
                        continue;
                    }
                    // Skip entities that start before the current span.
                    if loc.start_offset < context_len {
                        continue;
                    }
                    // Skip entities that extend beyond the current span.
                    if loc.start_offset - context_len >= span_len {
                        continue;
                    }
                    // Adjust offsets to be relative to the current span.
                    e.location = Some(Location::Text(TextLocation {
                        start_offset: loc.start_offset - context_len,
                        end_offset: loc.end_offset - context_len,
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }));
                } else {
                    // Non-text entity: set element_id via a new text location.
                    e.location = Some(Location::Text(TextLocation {
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }));
                }

                // Attach model info if provided.
                if let Some(ref model) = self.model_info {
                    e.model = Some(model.clone());
                }

                entities.push(e.with_parent(&span.source));
            }

            // Accumulate text for sliding context.
            let mut state = self.state.lock().await;
            if !state.prior_text.is_empty() {
                state.prior_text.push('\n');
            }
            state.prior_text.push_str(&span.data);
        }

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn parse_ner_entities_basic() {
        let raw = vec![json!({
            "category": "pii",
            "entity_type": "person_name",
            "value": "John Doe",
            "confidence": 0.95,
            "start_offset": 10,
            "end_offset": 18
        })];
        let entities = parse_ner_entities(&raw).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].value, "John Doe");
        assert_eq!(entities[0].entity_kind, EntityKind::PersonName);
        let loc = entities[0].location.as_ref().unwrap().as_text().unwrap();
        assert_eq!(loc.start_offset, 10);
        assert_eq!(loc.end_offset, 18);
    }

    #[test]
    fn parse_ner_entities_sets_element_id_none_by_default() {
        let raw = vec![json!({
            "category": "pii",
            "entity_type": "email_address",
            "value": "a@b.com",
            "confidence": 0.9,
            "start_offset": 0,
            "end_offset": 7
        })];
        let entities = parse_ner_entities(&raw).unwrap();
        let loc = entities[0].location.as_ref().unwrap().as_text().unwrap();
        assert!(loc.element_id.is_none());
    }

    /// Mock NER backend that returns entities relative to the full text it receives.
    struct MockNerBackend;

    #[async_trait::async_trait]
    impl NerBackend for MockNerBackend {
        async fn detect_text(
            &self,
            text: &str,
            _config: &NerConfig,
        ) -> Result<Vec<Value>, Error> {
            // Find "ENTITY" in the text and report its position.
            let mut results = Vec::new();
            if let Some(pos) = text.find("ENTITY") {
                results.push(json!({
                    "category": "pii",
                    "entity_type": "person_name",
                    "value": "ENTITY",
                    "confidence": 0.95,
                    "start_offset": pos,
                    "end_offset": pos + 6
                }));
            }
            Ok(results)
        }

        async fn detect_image(
            &self,
            _image_data: &[u8],
            _mime_type: &str,
            _config: &NerConfig,
        ) -> Result<Vec<Value>, Error> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn sliding_context_prepended_and_offsets_adjusted() {
        let params = NerDetectionParams {
            entity_kinds: vec![],
            confidence_threshold: 0.0,
            model_info: None,
        };
        let ner = NerDetection::new(MockNerBackend, params);

        // First span: no entity, just context.
        let span1 = vec![Span::new(TxtSpan(0), "some context text".into())];
        let result1 = ner.detect(span1).await.unwrap();
        assert!(result1.is_empty());

        // Second span: entity in current span. Backend sees prior + current.
        let span2 = vec![Span::new(TxtSpan(1), "has ENTITY here".into())];
        let result2 = ner.detect(span2).await.unwrap();
        assert_eq!(result2.len(), 1);

        // Offsets should be adjusted to current span (relative).
        let loc = result2[0].location.as_ref().unwrap().as_text().unwrap();
        assert_eq!(loc.start_offset, 4); // "has " = 4 chars
        assert_eq!(loc.end_offset, 10);  // "has ENTITY" = 10 chars
        assert_eq!(loc.element_id.as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn element_id_set_from_span() {
        let params = NerDetectionParams {
            entity_kinds: vec![],
            confidence_threshold: 0.0,
            model_info: None,
        };
        let ner = NerDetection::new(MockNerBackend, params);

        let spans = vec![Span::new(TxtSpan(42), "ENTITY".into())];
        let entities = ner.detect(spans).await.unwrap();
        assert_eq!(entities.len(), 1);
        let loc = entities[0].location.as_ref().unwrap().as_text().unwrap();
        assert_eq!(loc.element_id.as_deref(), Some("42"));
    }

    #[tokio::test]
    async fn model_info_attached_when_provided() {
        let model = ModelInfo {
            name: "test-model".into(),
            kind: crate::ModelKind::OpenSource,
            version: "1.0".into(),
        };
        let params = NerDetectionParams {
            entity_kinds: vec![],
            confidence_threshold: 0.0,
            model_info: Some(model.clone()),
        };
        let ner = NerDetection::new(MockNerBackend, params);

        let spans = vec![Span::new(TxtSpan(0), "ENTITY".into())];
        let entities = ner.detect(spans).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].model.as_ref().unwrap().name, "test-model");
    }

    #[tokio::test]
    async fn entities_in_prior_context_are_filtered_out() {
        // Backend that always returns an entity at position 0..6.
        struct AlwaysFirstBackend;

        #[async_trait::async_trait]
        impl NerBackend for AlwaysFirstBackend {
            async fn detect_text(
                &self,
                _text: &str,
                _config: &NerConfig,
            ) -> Result<Vec<Value>, Error> {
                Ok(vec![json!({
                    "category": "pii",
                    "entity_type": "person_name",
                    "value": "ENTITY",
                    "confidence": 0.95,
                    "start_offset": 0,
                    "end_offset": 6
                })])
            }

            async fn detect_image(
                &self,
                _: &[u8], _: &str, _: &NerConfig,
            ) -> Result<Vec<Value>, Error> {
                Ok(Vec::new())
            }
        }

        let params = NerDetectionParams {
            entity_kinds: vec![],
            confidence_threshold: 0.0,
            model_info: None,
        };
        let ner = NerDetection::new(AlwaysFirstBackend, params);

        // First span — entity at 0..6 in current span: should be included.
        let result1 = ner.detect(vec![Span::new(TxtSpan(0), "ENTITY here".into())]).await.unwrap();
        assert_eq!(result1.len(), 1);

        // Second span — entity at 0..6 is now in the prior context, should be filtered.
        let result2 = ner.detect(vec![Span::new(TxtSpan(1), "no entity".into())]).await.unwrap();
        assert!(result2.is_empty());
    }
}
