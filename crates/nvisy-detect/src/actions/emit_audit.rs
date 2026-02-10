use async_trait::async_trait;
use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::audit::Audit;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;
use nvisy_core::types::AuditAction;

pub struct EmitAuditAction;

#[async_trait]
impl Action for EmitAuditAction {
    fn id(&self) -> &str {
        "emit-audit"
    }

    fn input_type(&self) -> &str {
        "redaction"
    }

    fn output_type(&self) -> &str {
        "audit"
    }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<DataValue>,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        _client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, NvisyError> {
        let run_id: Option<uuid::Uuid> = params
            .get("runId")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let actor: Option<String> = params
            .get("actor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut count = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Redaction(redaction) = item {
                let mut audit = Audit::new(AuditAction::Redaction)
                    .with_entity_id(redaction.entity_id)
                    .with_redaction_id(redaction.data.id);

                if let Some(run_id) = run_id {
                    audit = audit.with_run_id(run_id);
                }
                if let Some(ref actor) = actor {
                    audit = audit.with_actor(actor);
                }

                let mut details = serde_json::Map::new();
                details.insert(
                    "method".to_string(),
                    serde_json::to_value(redaction.method).unwrap_or_default(),
                );
                details.insert(
                    "replacementValue".to_string(),
                    serde_json::Value::String(redaction.replacement_value.clone()),
                );
                if let Some(ref rule_id) = redaction.policy_rule_id {
                    details.insert(
                        "policyRuleId".to_string(),
                        serde_json::Value::String(rule_id.clone()),
                    );
                }
                audit = audit.with_details(details);

                audit.data.parent_id = Some(redaction.data.id);

                count += 1;
                if output.send(DataValue::Audit(audit)).await.is_err() {
                    return Ok(count);
                }
            }
        }

        Ok(count)
    }
}
