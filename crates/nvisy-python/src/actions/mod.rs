//! Pipeline actions that perform AI-powered named-entity recognition and OCR.
//!
//! Three actions are provided:
//! - [`DetectNerAction`] -- runs NER over text documents.
//! - [`DetectNerImageAction`] -- runs NER over images (OCR + entity detection).
//! - [`OcrDetectAction`] -- performs OCR on images to extract text regions.

/// OCR detection pipeline action.
pub mod ocr;

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::datatypes::document::ImageData;
use nvisy_core::error::Error;
use nvisy_core::registry::action::Action;
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
/// If the incoming [`Blob`] carries `"documents"` artifacts, each document's
/// text is sent through the NER model.  Otherwise the raw blob content is
/// interpreted as UTF-8 text.  Detected entities are stored as `"entities"`
/// artifacts on the blob.
pub struct DetectNerAction {
    /// Python bridge used to call the NER model.
    pub bridge: PythonBridge,
}

#[async_trait::async_trait]
impl Action for DetectNerAction {
    type Params = DetectNerParams;

    fn id(&self) -> &str { "detect-ner" }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let config = ner_config_from_params(&params);
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let documents: Vec<Document> = blob.get_artifacts("documents")
                .map_err(|e| Error::runtime(format!("Failed to get document artifacts: {}", e), "python/ner", false))?;

            let docs = if documents.is_empty() {
                let text = String::from_utf8(blob.content.to_vec())
                    .map_err(|e| Error::runtime(format!("Blob content is not valid UTF-8: {}", e), "python/ner", false))?;
                vec![Document::new(text)]
            } else {
                documents
            };

            for doc in &docs {
                let entities = ner::detect_ner(&self.bridge, &doc.content, &config).await?;
                for entity in &entities {
                    blob.add_artifact("entities", entity)
                        .map_err(|e| Error::runtime(format!("Failed to add entity artifact: {}", e), "python/ner", false))?;
                    count += 1;
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

/// Pipeline action that detects named entities in images.
///
/// If the incoming [`Blob`] carries `"images"` artifacts, each image is
/// processed individually.  Otherwise the raw blob content is treated as a
/// single image whose MIME type is inferred from the blob metadata.
/// Detected entities are stored as `"entities"` artifacts on the blob.
pub struct DetectNerImageAction {
    /// Python bridge used to call the NER model.
    pub bridge: PythonBridge,
}

#[async_trait::async_trait]
impl Action for DetectNerImageAction {
    type Params = DetectNerParams;

    fn id(&self) -> &str { "detect-ner-image" }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let config = ner_config_from_params(&params);
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let images: Vec<ImageData> = blob.get_artifacts("images")
                .map_err(|e| Error::runtime(format!("Failed to get image artifacts: {}", e), "python/ner-image", false))?;

            if images.is_empty() {
                let mime_type = blob.content_type().unwrap_or("application/octet-stream").to_string();
                let entities = ner::detect_ner_image(
                    &self.bridge,
                    &blob.content,
                    &mime_type,
                    &config,
                ).await?;
                for entity in &entities {
                    blob.add_artifact("entities", entity)
                        .map_err(|e| Error::runtime(format!("Failed to add entity artifact: {}", e), "python/ner-image", false))?;
                    count += 1;
                }
            } else {
                for img in &images {
                    let entities = ner::detect_ner_image(
                        &self.bridge,
                        &img.image_data,
                        &img.mime_type,
                        &config,
                    ).await?;
                    for entity in &entities {
                        blob.add_artifact("entities", entity)
                            .map_err(|e| Error::runtime(format!("Failed to add entity artifact: {}", e), "python/ner-image", false))?;
                        count += 1;
                    }
                }
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
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
