//! Top-level engine contract, I/O types, and the [`DefaultEngine`] implementation.
//!
//! The [`Engine`] trait defines the high-level redaction pipeline contract:
//! given content identifiers and an execution graph, produce redacted
//! output together with a full audit trail and per-phase breakdown.
//!
//! [`DefaultEngine`] is the standard implementation that orchestrates the
//! detect -> evaluate -> redact pipeline and drives the DAG execution graph.

mod analytics;
mod config;
mod default;
mod executor;
mod plan;
mod policy;
mod runs;

use std::future::Future;

use nvisy_core::Error;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::entity::DetectionOutput;
use nvisy_ontology::policy::{Policies, RedactionSummary};
use uuid::Uuid;

pub use self::analytics::{AnalyticsSnapshot, EngineAnalytics};
pub use self::config::{
    EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
pub use self::default::DefaultEngine;
pub use self::policy::{CompiledRetryPolicy, CompiledTimeoutPolicy};
pub use self::runs::{
    EngineRuns, NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
use crate::graph::Graph;
use crate::provenance::{Audit, PolicyEvaluation, RedactionMap};

/// Everything the caller must provide to run a redaction pipeline.
pub struct EngineInput {
    /// Human or service account identity.
    pub actor_id: Uuid,
    /// Identifiers of previously uploaded content to process.
    pub content_ids: Vec<Uuid>,
    /// Policies to apply (at least one).
    pub policies: Policies,
    /// Execution graph defining the pipeline DAG.
    pub graph: Graph,
    /// Reference-data contexts for detection.
    pub contexts: Contexts,
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

/// The top-level redaction engine contract.
///
/// Takes content identifiers and an execution graph; returns redacted
/// output, audit records, and a full breakdown of every pipeline phase.
pub trait Engine: Send + Sync {
    /// Execute a full redaction pipeline.
    fn run(&self, input: EngineInput) -> impl Future<Output = Result<EngineOutput, Error>> + Send;
}
