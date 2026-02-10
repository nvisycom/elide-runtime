use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A validated connection to an external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub credentials: serde_json::Value,
    #[serde(default)]
    pub context: serde_json::Value,
}

/// Map of connection_id -> Connection
pub type Connections = HashMap<String, Connection>;
