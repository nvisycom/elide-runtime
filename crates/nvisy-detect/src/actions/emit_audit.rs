//! Audit trail emission action.

use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use nvisy_core::datatypes::blob::Blob;
use nvisy_ontology::ontology::audit::{Audit, AuditAction};
use nvisy_ontology::ontology::redaction::Redaction;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// Typed parameters for [`EmitAuditAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitAuditParams {
    /// Pipeline run identifier to attach.
    #[serde(default)]
    pub run_id: Option<Uuid>,
    /// Human or service identity to record.
    #[serde(default)]
    pub actor: Option<String>,
}

/// Emits an [`Audit`] record for every [`Redaction`] found in the blob.
///
/// Each audit entry captures the redaction method, replacement value, and
/// (when available) the originating policy rule ID.
pub struct EmitAuditAction;

#[async_trait::async_trait]
impl Action for EmitAuditAction {
    type Params = EmitAuditParams;

    fn id(&self) -> &str {
        "emit-audit"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let run_id = params.run_id;
        let actor = params.actor;

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
