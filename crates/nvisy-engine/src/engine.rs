//! Top-level engine contract and I/O types.
//!
//! The [`Engine`] trait defines the high-level redaction pipeline contract:
//! given a content handler, policies, and an execution graph, produce redacted
//! output together with a full audit trail and per-phase breakdown.

use std::future::Future;

use uuid::Uuid;

use nvisy_core::error::Error;
use nvisy_core::fs::ContentHandler;
use nvisy_ontology::audit::Audit;
use nvisy_ontology::detection::{ClassificationResult, DetectionResult};
use nvisy_ontology::policy::{Policies, PolicyEvaluation};
use nvisy_ontology::redaction::RedactionSummary;

use crate::compiler::graph::Graph;
use crate::connections::Connections;
use crate::executor::runner::RunResult;

/// Everything the caller must provide to run a redaction pipeline.
pub struct EngineInput {
    /// Handle to the managed directory containing the files to process.
    pub source: ContentHandler,
    /// Policies to apply (at least one).
    pub policies: Policies,
    /// Execution graph defining the pipeline DAG.
    pub graph: Graph,
    /// External service connections for source/target nodes.
    pub connections: Connections,
    /// Human or service account identity.
    pub actor: Option<String>,
}

/// Full result of a pipeline run.
///
/// Contains a content handler for the redacted output, per-phase breakdown
/// (detection, classification, policy evaluation), per-source summaries,
/// audit records, and the raw DAG execution result.
pub struct EngineOutput {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Handle to the managed directory containing redacted output files.
    pub output: ContentHandler,
    /// Full detection result (entities, sensitivity, risk).
    pub detection: DetectionResult,
    /// Sensitivity classification.
    pub classification: ClassificationResult,
    /// Policy evaluation breakdown (redactions, reviews, suppressions, blocks, alerts).
    pub evaluation: PolicyEvaluation,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Immutable audit trail.
    pub audits: Vec<Audit>,
    /// Per-node execution results from the DAG runner.
    pub run_result: RunResult,
}

/// The top-level redaction engine contract.
///
/// Takes a content handler, policies, and an execution graph; returns redacted
/// output, audit records, and a full breakdown of every pipeline phase.
pub trait Engine: Send + Sync {
    /// Execute a full redaction pipeline.
    fn run(
        &self,
        input: EngineInput,
    ) -> impl Future<Output = Result<EngineOutput, Error>> + Send;
}
