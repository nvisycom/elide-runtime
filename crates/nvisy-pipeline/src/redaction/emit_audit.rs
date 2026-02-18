//! Audit trail emission action.

use jiff::Timestamp;
use serde::Deserialize;
use uuid::Uuid;

use nvisy_core::error::Error;
use nvisy_core::path::ContentSource;
use super::audit::{Audit, AuditAction};
use super::record::Redaction;

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

impl EmitAuditAction {
    pub async fn connect(params: EmitAuditParams) -> Result<Self, Error> {
        Ok(Self { params })
    }

    pub async fn execute(
        &self,
        redactions: Vec<Redaction>,
    ) -> Result<Vec<Audit>, Error> {
        let mut audits = Vec::new();

        for redaction in &redactions {
            let mut source = ContentSource::new();
            source.set_parent_id(Some(redaction.source.as_uuid()));

            let audit = Audit {
                source,
                action: AuditAction::Redaction,
                timestamp: Timestamp::now(),
                entity_id: Some(redaction.entity_id),
                redaction_id: Some(redaction.source.as_uuid()),
                policy_id: None,
                source_id: None,
                run_id: self.params.run_id,
                actor: self.params.actor.clone(),
            };

            audits.push(audit);
        }

        Ok(audits)
    }
}
