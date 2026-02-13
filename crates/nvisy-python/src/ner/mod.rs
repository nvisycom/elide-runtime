//! Named-entity recognition (NER) detection via a Python AI backend.
//!
//! Functions in this module acquire the GIL, call into the Python `nvisy_ai`
//! module, and convert the returned list of dicts into [`Entity`] values.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityLocation, TextLocation};
use nvisy_core::error::Error;
use crate::bridge::PythonBridge;
use crate::error::from_pyerr;

/// Configuration for NER detection passed to the Python backend.
#[derive(Debug, Clone)]
pub struct NerConfig {
    /// Entity type labels to detect (e.g., `["PERSON", "SSN"]`).
    pub entity_types: Vec<String>,
    /// Minimum confidence score to include a detection (0.0 -- 1.0).
    pub confidence_threshold: f64,
    /// Sampling temperature forwarded to the AI model.
    pub temperature: f64,
    /// API key for the AI provider.
    pub api_key: String,
    /// Model identifier (e.g., `"gpt-4"`).
    pub model: String,
    /// AI provider name (e.g., `"openai"`).
    pub provider: String,
}

/// Call Python detect_ner function via GIL + spawn_blocking.
pub async fn detect_ner(
    bridge: &PythonBridge,
    text: &str,
    config: &NerConfig,
) -> Result<Vec<Entity>, Error> {
    let module_name = bridge.module_name().to_string();
    let text = text.to_string();
    let config = config.clone();

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let module = py.import(&module_name).map_err(from_pyerr)?;

            let kwargs = PyDict::new(py);
            kwargs.set_item("text", &text).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &config.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", config.confidence_threshold).map_err(from_pyerr)?;
            kwargs.set_item("temperature", config.temperature).map_err(from_pyerr)?;
            kwargs.set_item("api_key", &config.api_key).map_err(from_pyerr)?;
            kwargs.set_item("model", &config.model).map_err(from_pyerr)?;
            kwargs.set_item("provider", &config.provider).map_err(from_pyerr)?;

            let result = module
                .call_method("detect_ner", (), Some(&kwargs))
                .map_err(from_pyerr)?;

            parse_python_entities(py, result)
        })
    })
    .await
    .map_err(|e| Error::python(format!("Task join error: {}", e)))?
}

/// Call Python detect_ner_image function via GIL + spawn_blocking.
pub async fn detect_ner_image(
    bridge: &PythonBridge,
    image_data: &[u8],
    mime_type: &str,
    config: &NerConfig,
) -> Result<Vec<Entity>, Error> {
    let module_name = bridge.module_name().to_string();
    let image_data = image_data.to_vec();
    let mime_type = mime_type.to_string();
    let config = config.clone();

    tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let module = py.import(&module_name).map_err(from_pyerr)?;

            let kwargs = PyDict::new(py);
            kwargs.set_item("image_bytes", &image_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("entity_types", &config.entity_types).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", config.confidence_threshold).map_err(from_pyerr)?;
            kwargs.set_item("api_key", &config.api_key).map_err(from_pyerr)?;
            kwargs.set_item("model", &config.model).map_err(from_pyerr)?;
            kwargs.set_item("provider", &config.provider).map_err(from_pyerr)?;

            let result = module
                .call_method("detect_ner_image", (), Some(&kwargs))
                .map_err(from_pyerr)?;

            parse_python_entities(py, result)
        })
    })
    .await
    .map_err(|e| Error::python(format!("Task join error: {}", e)))?
}

/// Parse Python list[dict] response into Vec<Entity>.
fn parse_python_entities(_py: Python<'_>, result: Bound<'_, PyAny>) -> Result<Vec<Entity>, Error> {
    let list: &Bound<'_, PyList> = result.downcast().map_err(|e| {
        Error::python(format!("Expected list from Python: {}", e))
    })?;

    let mut entities = Vec::new();

    for item in list.iter() {
        let dict: &Bound<'_, PyDict> = item.downcast().map_err(|e| {
            Error::python(format!("Expected dict in list: {}", e))
        })?;

        let category_str: String = dict
            .get_item("category")
            .map_err(from_pyerr)?
            .ok_or_else(|| Error::python("Missing 'category'"))?
            .extract()
            .map_err(from_pyerr)?;

        let category = match category_str.as_str() {
            "pii" => EntityCategory::Pii,
            "phi" => EntityCategory::Phi,
            "financial" => EntityCategory::Financial,
            "credentials" => EntityCategory::Credentials,
            other => EntityCategory::Custom(other.to_string()),
        };

        let entity_type: String = dict
            .get_item("entity_type")
            .map_err(from_pyerr)?
            .ok_or_else(|| Error::python("Missing 'entity_type'"))?
            .extract()
            .map_err(from_pyerr)?;

        let value: String = dict
            .get_item("value")
            .map_err(from_pyerr)?
            .ok_or_else(|| Error::python("Missing 'value'"))?
            .extract()
            .map_err(from_pyerr)?;

        let confidence: f64 = dict
            .get_item("confidence")
            .map_err(from_pyerr)?
            .ok_or_else(|| Error::python("Missing 'confidence'"))?
            .extract()
            .map_err(from_pyerr)?;

        let start_offset: usize = dict
            .get_item("start_offset")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0);

        let end_offset: usize = dict
            .get_item("end_offset")
            .map_err(from_pyerr)?
            .and_then(|v| v.extract().ok())
            .unwrap_or(0);

        let entity = Entity::new(
            category,
            entity_type,
            value,
            DetectionMethod::Ner,
            confidence,
            EntityLocation::Text(TextLocation {
                start_offset,
                end_offset,
                context_start_offset: None,
                context_end_offset: None,
                element_id: None,
                page_number: None,
            }),
        );

        entities.push(entity);
    }

    Ok(entities)
}
