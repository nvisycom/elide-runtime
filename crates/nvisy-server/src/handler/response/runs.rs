//! Run + per-document response wrappers.
//!
//! `RunResponse` is a thin projection over the engine's [`Run`]
//! header — it drops persistence-only fields (the recognition
//! plan, internal concurrency cap, the redundant `document_ids`
//! list) and inlines the per-doc bodies as a flat array. The
//! per-doc shape is the engine's [`RunDocument`] verbatim; entity
//! groups, locations, provenance, and reviewer overrides all
//! serialize through the engine types' own derives.

use std::collections::HashMap;

use jiff::Timestamp;
use nvisy_engine::runs::{ResourceRef, Run, RunDocument, RunState};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Run header + every per-document body, packaged as one
/// response. `GET /detections/{id}` and `GET /redactions/{id}`
/// both render through this — the caller filters by
/// [`state`](Self::state) to know which view they're in.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    /// Run id (same as the detection id and the redaction id —
    /// detections and redactions are filtered views of the same
    /// underlying run).
    pub id: Uuid,
    /// Top-level run state. The engine type flattens `state` and
    /// the state-specific fields (e.g. `reason` for `failed`) onto
    /// whatever struct holds it, so they sit at the response root.
    #[serde(flatten)]
    pub state: RunState,
    /// UUIDv7 timestamp the run was started.
    #[schemars(with = "String")]
    pub started_at: Timestamp,
    /// UUIDv7 timestamp of the most recent state transition.
    #[schemars(with = "String")]
    pub updated_at: Timestamp,
    /// Policies the caller submitted.
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts the caller submitted.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_refs: Vec<ResourceRef>,
    /// Per-request metadata.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    /// Per-document state. One entry per input file in the run.
    pub documents: Vec<RunDocument>,
}

impl RunResponse {
    /// Assemble a response from the run header + the per-doc
    /// rows the caller fetched (one fjall round-trip per id).
    pub fn assemble(run: Run, documents: Vec<RunDocument>) -> Self {
        RunResponse {
            id: run.id,
            state: run.state,
            started_at: run.started_at,
            updated_at: run.updated_at,
            policy_refs: run.policy_refs,
            context_refs: run.context_refs,
            metadata: run.metadata,
            documents,
        }
    }
}
