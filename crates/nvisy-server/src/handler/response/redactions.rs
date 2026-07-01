//! Redaction response shapes.

use nvisy_engine::runs::RunDocState;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Returned by `POST /redactions`. The id is the same run id
/// as the matching detection. `outputs` carries one entry per
/// input file in the run, so clients see which input produced
/// which redacted file and which per-doc applies failed.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionResult {
    /// Run id (= detection id).
    pub id: Uuid,
    /// One per input file. Position matches input order.
    pub outputs: Vec<RedactionOutput>,
}

/// One per-doc apply outcome.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionOutput {
    /// Doc id within the run.
    pub doc_id: Uuid,
    /// Input file the apply read.
    pub input_file_id: Uuid,
    /// Redacted output file, when this doc applied successfully.
    /// `None` when the doc failed — see [`state`](Self::state).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_id: Option<Uuid>,
    /// Per-doc lifecycle state after apply. Flattened — `state`
    /// and the state-specific fields (e.g. `reason` for `failed`)
    /// sit at this row's root.
    #[serde(flatten)]
    pub state: RunDocState,
}
