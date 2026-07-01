//! Request bodies for `/redactions` endpoints.

use nvisy_schema::policy::RuleAction;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Body for `POST /redactions`. References a prior detection
/// (the same run id) and optionally carries reviewer overrides
/// to apply per-entity before the redaction runs.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRedaction {
    /// Detection (run) to apply. The redaction reuses the same
    /// underlying run; the resulting redaction id equals
    /// [`detection_id`](Self::detection_id).
    pub detection_id: Uuid,
    /// Per-entity decision overrides. Applied in order before
    /// the redaction fan-out; an override on an entity id that
    /// doesn't exist in the run returns `NotFound`.
    #[serde(default)]
    pub overrides: Vec<NewOverride>,
}

/// One reviewer override: a per-entity action decided before
/// the redaction runs. The server loops these into
/// [`nvisy_engine::runs::override_entity`] before the apply
/// transition.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewOverride {
    /// Document id within the run.
    pub doc_id: Uuid,
    /// Entity id within the document.
    pub entity_id: Uuid,
    /// The action to apply.
    pub action: RuleAction,
}
