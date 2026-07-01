//! Policy response shapes.
//!
//! Reads return the [`nvisy_schema::policy::Policy`] type
//! directly (already `Serialize + JsonSchema`). `POST /policies`
//! returns the `(id, version)` summary so clients can reference
//! the policy without parsing the full body.

use schemars::JsonSchema;
use semver::Version;
use serde::Serialize;
use uuid::Uuid;

/// Summary returned by `POST /policies` and on each entry of
/// `GET /policies` list responses.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicySummary {
    /// Policy id.
    pub id: Uuid,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
}
