//! AI-powered named-entity recognition (NER) detection action.

use serde::Deserialize;
use serde_json::Value;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{TxtHandler, PngHandler};
use nvisy_core::entity::EntityCategory;
use nvisy_core::error::Error;

use crate::ontology::entity::{DetectionMethod, Entity, TextLocation};

fn default_confidence() -> f64 {
    0.5
}

/// Configuration passed to an [`NerBackend`] implementation.
///
/// Contains only the model-agnostic parameters that every backend needs.
/// Provider-specific fields (API key, model name, etc.) belong in the
/// action's [`DetectNerParams`] or the provider's credentials.
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
/// is handled by [`DetectNerAction`].
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

/// Typed parameters for [`DetectNerAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectNerParams {
    /// Entity types to detect (empty = all).
    #[serde(default)]
    pub entity_types: Vec<String>,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
}

/// Typed input for [`DetectNerAction`].
pub struct DetectNerInput {
    /// Text documents to scan for named entities.
    pub text_docs: Vec<Document<TxtHandler>>,
    /// Image documents to scan for named entities.
    pub image_docs: Vec<Document<PngHandler>>,
}

/// AI NER detection — delegates to an [`NerBackend`] at runtime.
pub struct DetectNerAction<B> {
    backend: B,
    params: DetectNerParams,
}

impl<B: NerBackend> DetectNerAction<B> {
    /// Create a new action with the given backend and params.
    pub fn new(backend: B, params: DetectNerParams) -> Self {
        Self { backend, params }
    }

    /// Build the [`NerConfig`] from action parameters.
    fn config(&self) -> NerConfig {
        NerConfig {
            entity_types: self.params.entity_types.clone(),
            confidence_threshold: self.params.confidence_threshold,
        }
    }

    /// Execute NER detection on text documents and image documents.
    pub async fn run(&self, input: DetectNerInput) -> Result<Vec<Entity>, Error> {
        let config = self.config();
        let mut entities = Vec::new();

        for doc in &input.text_docs {
            let text = doc.handler().lines().join("\n");
            let raw = self.backend.detect_text(&text, &config).await?;
            entities.extend(parse_ner_entities(&raw)?);
        }

        for doc in &input.image_docs {
            let png_bytes = doc.handler().encode_bytes()?;
            let raw = self
                .backend
                .detect_image(&png_bytes, "image/png", &config)
                .await?;
            entities.extend(parse_ner_entities(&raw)?);
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

        let entity_type = obj
            .get("entity_type")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'entity_type'".to_string()))?;

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
            entity_type,
            value,
            DetectionMethod::Ner,
            confidence,
        )
        .with_text_location(TextLocation {
            start_offset,
            end_offset,
            context_start_offset: None,
            context_end_offset: None,
            element_id: None,
            page_number: None,
        });

        entities.push(entity);
    }

    Ok(entities)
}
