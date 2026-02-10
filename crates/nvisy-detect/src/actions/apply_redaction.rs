use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::document::Document;
use nvisy_core::datatypes::entity::Entity;
use nvisy_core::datatypes::redaction::Redaction;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;

pub struct ApplyRedactionAction;

struct PendingRedaction {
    start_offset: usize,
    end_offset: usize,
    replacement_value: String,
}

#[async_trait]
impl Action for ApplyRedactionAction {
    fn id(&self) -> &str {
        "apply-redaction"
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
        let mut entities: HashMap<Uuid, Entity> = HashMap::new();
        let mut redactions: HashMap<Uuid, Redaction> = HashMap::new();
        let mut documents: Vec<Document> = Vec::new();

        // Collect all items first
        while let Some(item) = input.recv().await {
            match item {
                DataValue::Entity(e) => {
                    entities.insert(e.data.id, e);
                }
                DataValue::Redaction(r) => {
                    redactions.insert(r.entity_id, r);
                }
                DataValue::Document(d) => {
                    documents.push(d);
                }
                _ => {}
            }
        }

        let mut count = 0u64;

        for doc in documents {
            let mut pending: Vec<PendingRedaction> = Vec::new();

            for (entity_id, redaction) in &redactions {
                let entity = match entities.get(entity_id) {
                    Some(e) => e,
                    None => continue,
                };

                // Check entity belongs to this document
                let belongs = entity.data.parent_id == Some(doc.data.id)
                    || entity.source_id == Some(doc.data.id);
                if !belongs {
                    continue;
                }

                pending.push(PendingRedaction {
                    start_offset: entity.location.start_offset,
                    end_offset: entity.location.end_offset,
                    replacement_value: redaction.replacement_value.clone(),
                });
            }

            if pending.is_empty() {
                count += 1;
                if output.send(DataValue::Document(doc)).await.is_err() {
                    return Ok(count);
                }
                continue;
            }

            let redacted_content = apply_redactions(&doc.content, &mut pending);
            let mut result = Document::new(redacted_content);
            result.title = doc.title.clone();
            result.elements = doc.elements.clone();
            result.source_format = doc.source_format.clone();
            result.page_count = doc.page_count;
            result.data.parent_id = Some(doc.data.id);

            count += 1;
            if output.send(DataValue::Document(result)).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}

fn apply_redactions(text: &str, pending: &mut [PendingRedaction]) -> String {
    // Sort by start offset descending (right-to-left) to preserve positions
    pending.sort_by(|a, b| b.start_offset.cmp(&a.start_offset));

    let mut result = text.to_string();
    for redaction in pending.iter() {
        let start = redaction.start_offset.min(result.len());
        let end = redaction.end_offset.min(result.len());
        if start >= end {
            continue;
        }

        result = format!(
            "{}{}{}",
            &result[..start],
            redaction.replacement_value,
            &result[end..]
        );
    }
    result
}
