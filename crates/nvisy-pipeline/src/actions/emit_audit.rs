//! Audit trail emission action.

use serde::Deserialize;
use uuid::Uuid;

use nvisy_ontology::audit::{Audit, AuditAction};
use nvisy_ontology::redaction::Redaction;
use nvisy_core::error::Error;

use crate::action::Action;

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

/// Emits an [`Audit`] record for every [`Redaction`] provided.
///
/// Each audit entry captures the redaction method, replacement value, and
/// (when available) the originating policy rule ID.
pub struct EmitAuditAction {
    params: EmitAuditParams,
}

#[async_trait::async_trait]
impl Action for EmitAuditAction {
    type Params = EmitAuditParams;
    type Input = Vec<Redaction>;
    type Output = Vec<Audit>;

    fn id(&self) -> &str {
        "emit-audit"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        redactions: Self::Input,
    ) -> Result<Vec<Audit>, Error> {
        let run_id = self.params.run_id;
        let actor = &self.params.actor;

        let mut audits = Vec::new();

        for redaction in &redactions {
            let mut audit = Audit::new(AuditAction::Redaction)
                .with_entity_id(redaction.entity_id)
                .with_redaction_id(redaction.source.as_uuid());

            if let Some(run_id) = run_id {
                audit = audit.with_run_id(run_id);
            }
            if let Some(actor) = actor {
                audit = audit.with_actor(actor);
            }

            let mut details = serde_json::Map::new();
            details.insert(
                "output".to_string(),
                serde_json::to_value(&redaction.output).unwrap_or_default(),
            );
            if let Some(rule_id) = redaction.policy_rule_id {
                details.insert(
                    "policyRuleId".to_string(),
                    serde_json::Value::String(rule_id.to_string()),
                );
            }
            audit = audit.with_details(details);

            audit.source.set_parent_id(Some(redaction.source.as_uuid()));

            audits.push(audit);
        }

        Ok(audits)
    }
}
