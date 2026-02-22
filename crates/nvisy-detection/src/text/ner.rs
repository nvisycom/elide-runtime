//! AI-powered named-entity recognition (NER) detection layer.
//!
//! Uses a [`SequentialContext`] so the orchestrator feeds one span at
//! a time, allowing the layer to accumulate prior text/entities
//! between spans via interior mutability.

use std::str::FromStr;

use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;

use nvisy_codec::handler::{Span, TxtSpan};
use nvisy_core::data::{EntityCategory, EntityKind};
use nvisy_core::Error;
use nvisy_core::path::ContentSource;

use nvisy_python::bridge::PythonBridge;
use nvisy_python::ner::NerParams;

use crate::{DetectionMethod, Entity, TextLocation};
use crate::{SequentialContext, Detect, DetectionLayer};

fn default_confidence() -> f64 {
    0.5
}

/// Configuration passed to an [`NerBackend`] implementation.
///
/// Contains only the model-agnostic parameters that every backend needs.
/// Provider-specific fields (API key, model name, etc.) belong in the
/// action's [`NerDetectionParams`] or the provider's credentials.
#[derive(Debug, Clone)]
pub struct NerConfig {
    /// Entity type labels to detect (e.g., `["PERSON", "SSN"]`).
    pub entity_types: Vec<String>,
    /// Minimum confidence score to include a detection (0.0 -- 1.0).
    pub confidence_threshold: f64,
}

/// Backend trait for NER providers.
///
/// Implementations call an external NER service (e.g. via Python, HTTP)
/// and return raw JSON results.  Entity construction from the raw dicts
/// is handled by [`NerDetection`].
#[async_trait::async_trait]
pub trait NerBackend: Send + Sync + 'static {
    /// Detect entities in text, returning raw dicts.
    async fn detect_text(
        &self,
        text: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error>;

    /// Detect entities in an image, returning raw dicts.
    async fn detect_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error>;
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
impl<B: NerBackend> DetectionLayer for NerDetection<B> {
    type Params = NerDetectionParams;

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        // NerDetection requires a backend at construction time which
        // cannot be supplied via `connect`. Use `NerDetection::new`
        // instead.  This impl satisfies the trait bound but is not
        // the primary construction path.
        Err(Error::validation(
            "use NerDetection::new(backend, params) instead of connect",
            "detect-ner",
        ))
    }
}

#[async_trait::async_trait]
impl<B: NerBackend> Detect<TxtSpan, String> for NerDetection<B> {
    type Context = SequentialContext;

    async fn detect(
        &self,
        spans: Vec<Span<TxtSpan, String>>,
        source: &ContentSource,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            let raw = self
                .backend
                .detect_text(&span.data, &self.config)
                .await?;

            for e in parse_ner_entities(&raw)? {
                entities.push(
                    e.with_parent(source),
                );
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

/// Parse raw JSON dicts from an NER backend into [`Entity`] values.
///
/// Expected dict keys: `category`, `entity_type`, `value`, `confidence`,
/// and optionally `start_offset` / `end_offset`.
pub fn parse_ner_entities(raw: &[Value]) -> Result<Vec<Entity>, Error> {
    let mut entities = Vec::new();

    for item in raw {
        let obj = item.as_object().ok_or_else(|| {
            Error::python("Expected JSON object in NER results".to_string())
        })?;

        let category_str = obj
            .get("category")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'category'".to_string()))?;

        let category = match category_str {
            "pii" => EntityCategory::Pii,
            "phi" => EntityCategory::Phi,
            "financial" => EntityCategory::Financial,
            "credentials" => EntityCategory::Credentials,
            other => EntityCategory::Custom(other.to_string()),
        };

        let entity_type_str = obj
            .get("entity_type")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'entity_type'".to_string()))?;

        let entity_kind = match EntityKind::from_str(entity_type_str) {
            Ok(ek) => ek,
            Err(_) => {
                tracing::warn!(entity_type = entity_type_str, "unknown entity type from NER, dropping");
                continue;
            }
        };

        let value = obj
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'value'".to_string()))?;

        let confidence = obj
            .get("confidence")
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::python("Missing 'confidence'".to_string()))?;

        let start_offset = obj
            .get("start_offset")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(0);

        let end_offset = obj
            .get("end_offset")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(0);

        let entity = Entity::new(
            category,
            entity_kind,
            value,
            DetectionMethod::Ner,
            confidence,
        )
        .with_text_location(TextLocation {
            start_offset,
            end_offset,
            ..Default::default()
        });

        entities.push(entity);
    }

    Ok(entities)
}

/// [`NerBackend`] implementation for [`PythonBridge`].
///
/// Converts [`NerConfig`] to [`NerParams`] and delegates to `nvisy_python::ner`.
#[async_trait::async_trait]
impl NerBackend for PythonBridge {
    async fn detect_text(
        &self,
        text: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error> {
        let params = NerParams {
            entity_types: config.entity_types.clone(),
            confidence_threshold: config.confidence_threshold,
        };
        nvisy_python::ner::detect_ner(self, text, &params).await
    }

    async fn detect_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &NerConfig,
    ) -> Result<Vec<Value>, Error> {
        let params = NerParams {
            entity_types: config.entity_types.clone(),
            confidence_threshold: config.confidence_threshold,
        };
        nvisy_python::ner::detect_ner_image(self, image_data, mime_type, &params).await
    }
}
