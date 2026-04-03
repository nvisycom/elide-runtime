//! Run response types.

use nvisy_engine::pipeline::{EngineOutput, RunEntry};
use nvisy_ontology::provenance::Audit;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

use crate::handler::request::Page;

/// Response body for `GET /runs`.
pub type RunList = Page<RunEntry>;

/// Response body for `POST /runs`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// Identifier assigned to this pipeline run.
    pub run_id: Uuid,
    /// Per-document audit trails.
    pub audits: Vec<Audit>,
    /// Total number of entities detected across all documents.
    pub total_entities: usize,
    /// Total number of redaction entries produced.
    pub total_entries: usize,
}

impl From<EngineOutput> for RunResult {
    fn from(output: EngineOutput) -> Self {
        let total_entities = output.audits.iter().map(|a| a.entities.len()).sum();
        let total_entries = output.audits.iter().map(|a| a.entries.len()).sum();
        Self {
            run_id: output.run_id,
            audits: output.audits,
            total_entities,
            total_entries,
        }
    }
}
