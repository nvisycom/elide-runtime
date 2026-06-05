//! `Policy<Text>` response types.

use nvisy_core::modality::Text;
use nvisy_document::policy::Policy;
use schemars::JsonSchema;
use semver::Version;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::request::Page;

/// Response body for `GET /policies`.
pub type PolicyList = Page<PolicyEntry>;

/// Summary of a stored policy for listing endpoints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEntry {
    /// Identifier of the policy.
    pub id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy<Text> version.
    #[schemars(with = "String")]
    pub version: Version,
}

impl From<Policy<Text>> for PolicyEntry {
    fn from(policy: Policy<Text>) -> Self {
        Self {
            id: policy.id,
            name: policy.name,
            version: policy.version,
        }
    }
}

/// Response body for `POST /policies`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyId {
    /// Identifier assigned to the uploaded policy.
    pub id: Uuid,
}
