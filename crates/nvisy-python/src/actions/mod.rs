//! Pipeline actions that perform AI-powered named-entity recognition and OCR.
//!
//! Three actions are provided:
//! - [`DetectNerAction`] -- runs NER over text documents.
//! - [`DetectNerImageAction`] -- runs NER over images (OCR + entity detection).
//! - [`OcrDetectAction`] -- performs OCR on images to extract text regions.

/// OCR detection pipeline action.
#[cfg(feature = "png")]
pub mod ocr;

use serde::Deserialize;

use nvisy_codec::handler::{TxtHandler, TxtData};
#[cfg(feature = "png")]
use nvisy_codec::handler::PngHandler;
use nvisy_codec::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_core::error::Error;
use nvisy_core::io::ContentData;
use nvisy_pipeline::action::Action;
use crate::bridge::PythonBridge;
use crate::ner::{self, NerConfig};

/// Typed parameters for NER actions.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectNerParams {
    /// Entity type labels to detect (e.g., `["PERSON", "SSN"]`).
    #[serde(default)]
    pub entity_types: Vec<String>,
    /// Minimum confidence score to include a detection (0.0 -- 1.0).
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f64,
    /// Sampling temperature forwarded to the AI model.
    #[serde(default)]
    pub temperature: f64,
    /// API key for the AI provider.
    #[serde(default)]
    pub api_key: String,
    /// Model identifier (e.g., `"gpt-4"`).
    #[serde(default = "default_model")]
    pub model: String,
    /// AI provider name (e.g., `"openai"`).
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_confidence_threshold() -> f64 { 0.5 }
fn default_model() -> String { "gpt-4".to_string() }
fn default_provider() -> String { "openai".to_string() }

/// Pipeline action that detects named entities in text documents.
///
/// Each document's text is sent through the NER model. If no documents are
/// provided, the raw content is interpreted as UTF-8 text. Detected entities
/// are returned directly.
pub struct DetectNerAction {
    /// Python bridge used to call the NER model.
    pub bridge: PythonBridge,
    params: DetectNerParams,
}

impl DetectNerAction {
    /// Replace the default bridge with a pre-configured one.
    pub fn with_bridge(mut self, bridge: PythonBridge) -> Self {
        self.bridge = bridge;
        self
    }
}

#[async_trait::async_trait]
impl Action for DetectNerAction {
    type Params = DetectNerParams;
    type Input = (ContentData, Vec<Document<TxtHandler>>);
    type Output = Vec<Entity>;

    fn id(&self) -> &str { "detect-ner" }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { bridge: PythonBridge::default(), params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let (content, documents) = input;
        let config = ner_config_from_params(&self.params);

        let docs = if documents.is_empty() {
            let text = content.as_str()
                .map_err(|e| Error::runtime(
                    format!("Content is not valid UTF-8: {}", e),
                    "python/ner",
                    false,
                ))?;
            let handler = TxtHandler::new(TxtData {
                lines: text.lines().map(String::from).collect(),
                trailing_newline: text.ends_with('\n'),
            });
            vec![Document::new(handler)]
        } else {
            documents
        };

        let mut all_entities = Vec::new();
        for doc in &docs {
            let text = doc.handler().lines().join("\n");
            let entities = ner::detect_ner(&self.bridge, &text, &config).await?;
            all_entities.extend(entities);
        }

        Ok(all_entities)
    }
}

/// Pipeline action that detects named entities in images.
///
/// Each image is processed individually through NER. If no images are
/// provided, the raw content is treated as a single image whose MIME type
/// is inferred from the content metadata. Detected entities are returned
/// directly.
#[cfg(feature = "png")]
pub struct DetectNerImageAction {
    /// Python bridge used to call the NER model.
    pub bridge: PythonBridge,
    params: DetectNerParams,
}

#[cfg(feature = "png")]
impl DetectNerImageAction {
    /// Replace the default bridge with a pre-configured one.
    pub fn with_bridge(mut self, bridge: PythonBridge) -> Self {
        self.bridge = bridge;
        self
    }
}

#[cfg(feature = "png")]
#[async_trait::async_trait]
impl Action for DetectNerImageAction {
    type Params = DetectNerParams;
    type Input = (ContentData, Vec<Document<PngHandler>>);
    type Output = Vec<Entity>;

    fn id(&self) -> &str { "detect-ner-image" }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { bridge: PythonBridge::default(), params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        let (content, images) = input;
        let config = ner_config_from_params(&self.params);

        let mut all_entities = Vec::new();

        if images.is_empty() {
            let mime_type = content.content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let entities = ner::detect_ner_image(
                &self.bridge,
                content.as_bytes(),
                &mime_type,
                &config,
            ).await?;
            all_entities.extend(entities);
        } else {
            for doc in &images {
                let entities = ner::detect_ner_image(
                    &self.bridge,
                    doc.handler().bytes(),
                    "image/png",
                    &config,
                ).await?;
                all_entities.extend(entities);
            }
        }

        Ok(all_entities)
    }
}

/// Convert [`DetectNerParams`] into the internal [`NerConfig`].
fn ner_config_from_params(params: &DetectNerParams) -> NerConfig {
    NerConfig {
        entity_types: params.entity_types.clone(),
        confidence_threshold: params.confidence_threshold,
        temperature: params.temperature,
        api_key: params.api_key.clone(),
        model: params.model.clone(),
        provider: params.provider.clone(),
    }
}
