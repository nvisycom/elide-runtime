use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredAudit {
    pub id: Uuid,
    pub action: String,
    pub timestamp: String,
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
    pub details: Option<serde_json::Value>,
}

pub struct AuditStore {
    records: RwLock<Vec<StoredAudit>>,
}

impl AuditStore {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(Vec::new()),
        }
    }

    pub fn add(&self, record: StoredAudit) {
        self.records.write().unwrap().push(record);
    }

    pub fn query(
        &self,
        run_id: Option<&str>,
        action: Option<&str>,
        source_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Vec<StoredAudit> {
        let records = self.records.read().unwrap();
        let mut results: Vec<&StoredAudit> = records.iter().collect();

        if let Some(rid) = run_id {
            if let Ok(uid) = rid.parse::<Uuid>() {
                results.retain(|r| r.run_id == Some(uid));
            }
        }
        if let Some(act) = action {
            results.retain(|r| r.action == act);
        }
        if let Some(sid) = source_id {
            if let Ok(uid) = sid.parse::<Uuid>() {
                results.retain(|r| r.source_id == Some(uid));
            }
        }

        results.into_iter().skip(offset).take(limit).cloned().collect()
    }

    pub fn get_by_run_id(&self, run_id: Uuid) -> Vec<StoredAudit> {
        let records = self.records.read().unwrap();
        records.iter().filter(|r| r.run_id == Some(run_id)).cloned().collect()
    }
}
