//! Audit trail emission action.

use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::audit::Audit;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::traits::action::Action;
use nvisy_core::datatypes::audit::AuditAction;
use nvisy_core::datatypes::redaction::Redaction;

/// Emits an [`Audit`] record for every [`Redaction`] found in the blob.
///
/// Each audit entry captures the redaction method, replacement value, and
/// (when available) the originating policy rule ID. Optional `runId` and
/// `actor` parameters are attached to every emitted audit.
///
/// # Parameters (JSON)
///
/// | Key     | Type     | Default | Description                         |
/// |---------|----------|---------|-------------------------------------|
/// | `runId` | `UUID`   | `None`  | Pipeline run identifier to attach.  |
/// | `actor` | `String` | `None`  | Human or service identity to record.|
pub struct EmitAuditAction;

#[async_trait::async_trait]
impl Action for EmitAuditAction {
    fn id(&self) -> &str {
        "emit-audit"
    }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        _client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, Error> {
        let run_id: Option<uuid::Uuid> = params
            .get("runId")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let actor: Option<String> = params
            .get("actor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let redactions: Vec<Redaction> = blob.get_artifacts("redactions").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read redactions artifact: {e}"))
            })?;

            for redaction in &redactions {
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

                blob.add_artifact("audits", &audit).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add audit artifact: {e}"))
                })?;

                count += 1;
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}
