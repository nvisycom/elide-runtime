//! `(id, version)` reference shape used by `POST /detections`
//! for `policyRefs` and `contextRefs`. JsonSchema-wrapped mirror
//! of [`nvisy_engine::runs::ResourceRef`] (engine type doesn't
//! derive `JsonSchema`).

use nvisy_engine::runs::ResourceRef as EngineRef;
use schemars::JsonSchema;
use semver::Version;
use serde::Deserialize;
use uuid::Uuid;

/// Reference to a stored policy or context resource by
/// `(id, version)`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    /// Resource UUID.
    pub id: Uuid,
    /// Resource version.
    #[schemars(with = "String")]
    pub version: Version,
}

impl From<ResourceRef> for EngineRef {
    fn from(r: ResourceRef) -> Self {
        EngineRef {
            id: r.id,
            version: r.version,
        }
    }
}
