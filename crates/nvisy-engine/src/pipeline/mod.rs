//! Top-level engine types and the [`Engine`] implementation.
//!
//! [`Engine`] orchestrates the detect -> evaluate -> redact pipeline
//! and drives the DAG execution graph.

mod analytics;
mod config;
mod default;
mod executor;
mod orchestrator;
mod plan;
mod runs;

use nvisy_ontology::entity::DetectionOutput;
use nvisy_ontology::policy::{Policies, RedactionSummary};
use uuid::Uuid;

pub use self::analytics::AnalyticsSnapshot;
pub use self::config::{
    EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
pub use self::default::Engine;
pub use self::runs::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
use crate::graph::Graph;
use crate::provenance::{Audit, PolicyEvaluation, RedactionMap};

/// Everything the caller must provide to run a redaction pipeline.
pub struct EngineInput {
    /// Human or service account identity.
    pub actor_id: Uuid,
    /// Policies to apply (at least one).
    pub policies: Policies,
    /// Execution graph defining the pipeline DAG.
    ///
    /// Content identifiers live on [`ImportFile`] nodes within the graph,
    /// not as a top-level field.
    ///
    /// [`ImportFile`]: crate::graph::ImportFile
    pub graph: Graph,
    /// Per-request configuration overrides (merged with engine defaults).
    pub config: Option<RuntimeConfig>,
}

/// Full result of a pipeline run.
///
/// Contains per-phase breakdown (detection, classification, policy evaluation),
/// per-source summaries, and audit records.
pub struct EngineOutput {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Full detection result (entities, sensitivity, risk).
    pub detection: DetectionOutput,
    /// Policy evaluation breakdown (redactions, reviews, suppressions, blocks, alerts).
    pub evaluation: PolicyEvaluation,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Per-file processing logs.
    pub file_audits: Vec<Audit>,
    /// Redaction mapping artifacts.
    pub redaction_maps: Vec<RedactionMap>,
}
