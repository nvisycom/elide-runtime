//! Detection response shapes.
//!
//! Most `/detections` reads return the shared [`RunResponse`]
//! (header + per-doc bodies); see [`super::runs`]. This module
//! holds the only detection-specific shape, the just-created
//! id returned by `POST /detections`.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Returned by `POST /detections`. The id is the new run id —
/// the same id the matching `POST /redactions` will reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionId {
    /// Engine-minted UUIDv7.
    pub id: Uuid,
}
