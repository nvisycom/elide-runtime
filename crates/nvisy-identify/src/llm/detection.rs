//! LLM contextual detection layer.
//!
//! Uses a [`SequentialContext`] so the orchestrator feeds one span at
//! a time, allowing the layer to accumulate prior text for contextual
//! understanding across spans.

use serde::Deserialize;
use tokio::sync::Mutex;

use nvisy_codec::handler::{Span, TxtSpan};
use nvisy_ontology::entity::EntityKind;
use nvisy_core::Error;
use nvisy_rig::{LlmBackend, LlmConfig, parse_llm_entities};

use crate::{Entity, Location, ModelInfo, TextLocation};
use crate::{SequentialContext, DetectionService};

use super::prompt;

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`LlmDetection`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmDetectionParams {
    /// Entity kinds to detect (empty = all).
    #[serde(rename = "entityTypes", default)]
    pub entity_kinds: Vec<EntityKind>,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
    /// Optional model info to attach to every LLM-produced entity.
    #[serde(skip)]
    pub model_info: Option<ModelInfo>,
    /// Optional system prompt override.
    #[serde(default)]
    pub system_prompt: Option<String>,
}

/// Accumulated state between sequential span calls.
struct LlmState {
    /// Text from previously processed spans (for sliding context).
    prior_text: String,
}

/// LLM contextual detection layer — delegates to an [`LlmBackend`].
///
/// Uses [`SequentialContext`]: the orchestrator feeds one span at a
/// time so the layer can carry sliding context between spans.
pub struct LlmDetection<B> {
    backend: B,
    config: LlmConfig,
    model_info: Option<ModelInfo>,
    state: Mutex<LlmState>,
}

impl<B: LlmBackend> LlmDetection<B> {
    /// Create a new detection layer with the given backend and params.
    pub fn new(backend: B, params: LlmDetectionParams) -> Self {
        let system_prompt = params.system_prompt.unwrap_or_else(|| {
            prompt::system_prompt().to_string()
        });
        let config = LlmConfig {
            entity_types: params.entity_kinds.iter().map(|ek| ek.to_string()).collect(),
            confidence_threshold: params.confidence_threshold,
            system_prompt: Some(system_prompt),
        };
        Self {
            backend,
            config,
            model_info: params.model_info,
            state: Mutex::new(LlmState {
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
impl<B: LlmBackend> DetectionService<TxtSpan, String> for LlmDetection<B> {
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
            for mut e in parse_llm_entities(&raw)? {
                if let Some(Location::Text(ref loc)) = e.location {
                    if loc.end_offset <= context_len {
                        continue;
                    }
                    if loc.start_offset < context_len {
                        continue;
                    }
                    if loc.start_offset - context_len >= span_len {
                        continue;
                    }
                    e.location = Some(Location::Text(TextLocation {
                        start_offset: loc.start_offset - context_len,
                        end_offset: loc.end_offset - context_len,
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }));
                } else {
                    e.location = Some(Location::Text(TextLocation {
                        element_id: Some(span.id.0.to_string()),
                        ..Default::default()
                    }));
                }

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

    struct MockLlmBackend;

    #[async_trait::async_trait]
    impl LlmBackend for MockLlmBackend {
        async fn detect_text(
            &self,
            text: &str,
            _config: &LlmConfig,
        ) -> Result<Vec<Value>, Error> {
            let mut results = Vec::new();
            if let Some(pos) = text.find("SECRET") {
                results.push(json!({
                    "category": "credentials",
                    "entity_type": "api_key",
                    "value": "SECRET",
                    "confidence": 0.92,
                    "start_offset": pos,
                    "end_offset": pos + 6
                }));
            }
            Ok(results)
        }
    }

    #[tokio::test]
    async fn llm_detection_basic() {
        let params = LlmDetectionParams {
            entity_kinds: vec![],
            confidence_threshold: 0.0,
            model_info: None,
            system_prompt: None,
        };
        let llm = LlmDetection::new(MockLlmBackend, params);

        let spans = vec![Span::new(TxtSpan(0), "contains SECRET key".into())];
        let entities = llm.detect(spans).await.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].value, "SECRET");

        let loc = entities[0].location.as_ref().unwrap().as_text().unwrap();
        assert_eq!(loc.start_offset, 9);
        assert_eq!(loc.end_offset, 15);
    }

    #[tokio::test]
    async fn llm_detection_with_context() {
        let params = LlmDetectionParams {
            entity_kinds: vec![],
            confidence_threshold: 0.0,
            model_info: None,
            system_prompt: None,
        };
        let llm = LlmDetection::new(MockLlmBackend, params);

        // First span: no entity.
        let span1 = vec![Span::new(TxtSpan(0), "some context".into())];
        let result1 = llm.detect(span1).await.unwrap();
        assert!(result1.is_empty());

        // Second span: entity in current span.
        let span2 = vec![Span::new(TxtSpan(1), "has SECRET here".into())];
        let result2 = llm.detect(span2).await.unwrap();
        assert_eq!(result2.len(), 1);

        let loc = result2[0].location.as_ref().unwrap().as_text().unwrap();
        assert_eq!(loc.start_offset, 4);
        assert_eq!(loc.end_offset, 10);
    }
}
