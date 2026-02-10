use serde::{Deserialize, Serialize};

/// Category of sensitive data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EntityCategory {
    Pii,
    Phi,
    Financial,
    Credentials,
    Custom,
}

/// How the entity was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    Regex,
    AiNer,
    Dictionary,
    Checksum,
    Composite,
}

/// Method used to redact sensitive data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RedactionMethod {
    Mask,
    Replace,
    Hash,
    Encrypt,
    Remove,
    Blur,
    Block,
    Synthesize,
}

/// Type of auditable action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Detection,
    Redaction,
    PolicyEval,
    Access,
    Export,
}

/// General-purpose metadata map.
pub type Metadata = serde_json::Map<String, serde_json::Value>;
