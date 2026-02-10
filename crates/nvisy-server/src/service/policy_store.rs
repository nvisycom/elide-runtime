use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredPolicy {
    pub id: Uuid,
    pub name: String,
    pub rules: Vec<serde_json::Value>,
    pub default_method: String,
    pub default_confidence_threshold: f64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PolicyStore {
    policies: RwLock<HashMap<Uuid, StoredPolicy>>,
}

impl PolicyStore {
    pub fn new() -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
        }
    }

    pub fn create(
        &self,
        name: String,
        rules: Vec<serde_json::Value>,
        default_method: String,
        default_confidence_threshold: f64,
    ) -> StoredPolicy {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();
        let policy = StoredPolicy {
            id,
            name,
            rules,
            default_method,
            default_confidence_threshold,
            created_at: now.clone(),
            updated_at: now,
        };
        self.policies.write().unwrap().insert(id, policy.clone());
        policy
    }

    pub fn get(&self, id: Uuid) -> Option<StoredPolicy> {
        self.policies.read().unwrap().get(&id).cloned()
    }

    pub fn list(&self) -> Vec<StoredPolicy> {
        self.policies.read().unwrap().values().cloned().collect()
    }

    pub fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        rules: Option<Vec<serde_json::Value>>,
        default_method: Option<String>,
        default_confidence_threshold: Option<f64>,
    ) -> Option<StoredPolicy> {
        let mut policies = self.policies.write().unwrap();
        let existing = policies.get_mut(&id)?;
        if let Some(n) = name { existing.name = n; }
        if let Some(r) = rules { existing.rules = r; }
        if let Some(m) = default_method { existing.default_method = m; }
        if let Some(t) = default_confidence_threshold { existing.default_confidence_threshold = t; }
        existing.updated_at = chrono::Utc::now().to_rfc3339();
        Some(existing.clone())
    }

    pub fn delete(&self, id: Uuid) -> bool {
        self.policies.write().unwrap().remove(&id).is_some()
    }
}
