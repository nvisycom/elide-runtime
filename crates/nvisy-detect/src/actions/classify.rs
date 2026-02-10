use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::document::Document;
use nvisy_core::datatypes::entity::Entity;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;

pub struct ClassifyAction;

#[async_trait]
impl Action for ClassifyAction {
    fn id(&self) -> &str {
        "classify"
    }

    fn input_type(&self) -> &str {
        "document"
    }

    fn output_type(&self) -> &str {
        "document"
    }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<DataValue>,
        output: mpsc::Sender<DataValue>,
        _params: serde_json::Value,
        _client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, NvisyError> {
        let mut entities_by_source: HashMap<uuid::Uuid, Vec<Entity>> = HashMap::new();
        let mut documents: Vec<Document> = Vec::new();

        while let Some(item) = input.recv().await {
            match item {
                DataValue::Entity(e) => {
                    let source_id = e.data.parent_id.unwrap_or(uuid::Uuid::nil());
                    entities_by_source.entry(source_id).or_default().push(e);
                }
                DataValue::Document(d) => {
                    documents.push(d);
                }
                _ => {}
            }
        }

        let mut count = 0u64;

        for doc in documents {
            let entities = entities_by_source
                .get(&doc.data.id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let sensitivity_level = compute_sensitivity_level(entities);

            let mut result = Document::new(&doc.content);
            result.title = doc.title.clone();
            result.elements = doc.elements.clone();
            result.source_format = doc.source_format.clone();
            result.page_count = doc.page_count;
            result.data.parent_id = Some(doc.data.id);

            let mut meta = doc.data.metadata.clone().unwrap_or_default();
            meta.insert(
                "sensitivityLevel".to_string(),
                serde_json::Value::String(sensitivity_level),
            );
            meta.insert(
                "totalEntities".to_string(),
                serde_json::Value::Number(entities.len().into()),
            );
            result.data.metadata = Some(meta);

            count += 1;
            if output.send(DataValue::Document(result)).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

fn compute_sensitivity_level(entities: &[Entity]) -> String {
    if entities.is_empty() {
        return "none".to_string();
    }

    let has_high_confidence = entities.iter().any(|e| e.confidence >= 0.9);
    let has_critical_types = entities.iter().any(|e| {
        matches!(e.category, nvisy_core::types::EntityCategory::Credentials)
            || e.entity_type == "ssn"
            || e.entity_type == "credit_card"
    });

    if has_critical_types && has_high_confidence {
        return "critical".to_string();
    }
    if has_critical_types || entities.len() > 10 {
        return "high".to_string();
    }
    if entities.len() > 3 {
        return "medium".to_string();
    }
    "low".to_string()
}
