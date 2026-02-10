use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::data::DataItem;
use crate::types::{AuditAction, Metadata};

/// An immutable audit record tracking a data protection event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audit {
    #[serde(flatten)]
    pub data: DataItem,
    pub action: AuditAction,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redaction_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Metadata>,
}

impl Audit {
    pub fn new(action: AuditAction) -> Self {
        Self {
            data: DataItem::new(),
            action,
            timestamp: Utc::now(),
            entity_id: None,
            redaction_id: None,
            policy_id: None,
            source_id: None,
            run_id: None,
            actor: None,
            details: None,
        }
    }

    pub fn with_entity_id(mut self, id: Uuid) -> Self {
        self.entity_id = Some(id);
        self
    }

    pub fn with_redaction_id(mut self, id: Uuid) -> Self {
        self.redaction_id = Some(id);
        self
    }

    pub fn with_run_id(mut self, id: Uuid) -> Self {
        self.run_id = Some(id);
        self
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    pub fn with_details(mut self, details: Metadata) -> Self {
        self.details = Some(details);
        self
    }
}
