//! Context response shapes. Symmetric with [`super::policies`].

use schemars::JsonSchema;
use semver::Version;
use serde::Serialize;
use uuid::Uuid;

/// Summary returned by `POST /contexts` and on each entry of
/// `GET /contexts` list responses.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextSummary {
    /// Context id.
    pub id: Uuid,
    /// Context version.
    #[schemars(with = "String")]
    pub version: Version,
}
