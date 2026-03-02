//! Credential reference data for secret detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A reference credential for detecting leaked secrets.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialData {
    /// The credential value (API key, token, etc.).
    pub value: String,
    /// Type of credential (e.g. `"api_key"`, `"oauth_token"`, `"password"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_type: Option<String>,
    /// Service or provider this credential belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}
