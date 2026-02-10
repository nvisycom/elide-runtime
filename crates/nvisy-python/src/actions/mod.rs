use std::any::Any;
use async_trait::async_trait;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;
use crate::bridge::PythonBridge;
use crate::ner::{self, NerConfig};

/// AI NER detection action for text documents.
pub struct DetectNerAction;

#[async_trait]
impl Action for DetectNerAction {
    fn id(&self) -> &str { "detect-ner" }
    fn input_type(&self) -> &str { "document" }
    fn output_type(&self) -> &str { "entity" }
    fn requires_client(&self) -> bool { true }
    fn required_provider_id(&self) -> Option<&str> { Some("ai") }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<DataValue>,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, NvisyError> {
        let bridge = extract_bridge(client)?;
        let config = parse_ner_config(&params);
        let mut count = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Document(doc) = &item {
                let entities = ner::detect_ner(&bridge, &doc.content, &config).await?;
                for mut entity in entities {
                    entity.source_id = Some(doc.data.id);
                    entity.data.parent_id = Some(doc.data.id);
                    count += 1;
                    if output.send(DataValue::Entity(entity)).await.is_err() {
                        return Ok(count);
                    }
                }
            }
        }

        Ok(count)
    }
}

/// AI NER detection action for images.
pub struct DetectNerImageAction;

#[async_trait]
impl Action for DetectNerImageAction {
    fn id(&self) -> &str { "detect-ner-image" }
    fn input_type(&self) -> &str { "image" }
    fn output_type(&self) -> &str { "entity" }
    fn requires_client(&self) -> bool { true }
    fn required_provider_id(&self) -> Option<&str> { Some("ai") }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<DataValue>,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, NvisyError> {
        let bridge = extract_bridge(client)?;
        let config = parse_ner_config(&params);
        let mut count = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Image(img) = &item {
                let entities = ner::detect_ner_image(
                    &bridge,
                    &img.image_data,
                    &img.mime_type,
                    &config,
                ).await?;
                for mut entity in entities {
                    entity.data.parent_id = Some(img.data.id);
                    count += 1;
                    if output.send(DataValue::Entity(entity)).await.is_err() {
                        return Ok(count);
                    }
                }
            }
        }

        Ok(count)
    }
}

fn extract_bridge(client: Option<Box<dyn Any + Send>>) -> Result<PythonBridge, NvisyError> {
    client
        .ok_or_else(|| NvisyError::runtime("AI provider client required", "python", false))?
        .downcast::<PythonBridge>()
        .map(|b| *b)
        .map_err(|_| NvisyError::runtime("Invalid client type for AI actions", "python", false))
}

fn parse_ner_config(params: &serde_json::Value) -> NerConfig {
    NerConfig {
        entity_types: params
            .get("entityTypes")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        confidence_threshold: params
            .get("confidenceThreshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5),
        temperature: params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        api_key: params
            .get("apiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: params
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-4")
            .to_string(),
        provider: params
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai")
            .to_string(),
    }
}
