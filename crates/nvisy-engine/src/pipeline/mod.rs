//! Top-level engine contract, I/O types, and the [`DefaultEngine`] implementation.
//!
//! The [`Engine`] trait defines the high-level redaction pipeline contract:
//! given a content handler, policies, and an execution graph, produce redacted
//! output together with a full audit trail and per-phase breakdown.
//!
//! [`DefaultEngine`] is the standard implementation that orchestrates the
//! detect -> evaluate -> redact pipeline and drives the DAG execution graph.

mod connections;
mod default;
mod executor;
mod ontology;
mod policy;
mod runs;

pub use connections::{Connection, Connections};
pub use default::DefaultEngine;
pub use executor::{NodeOutput, RunOutput};
pub use ontology::{Explainable, Explanation};
pub use runs::{NodeProgress, RunManager, RunState, RunStatus, RunSummary};

use std::future::Future;

use uuid::Uuid;

use nvisy_core::Error;
use nvisy_core::fs::ContentHandler;
pub use nvisy_ontology::policy::{Policies, PolicyEvaluation, RedactionSummary};
pub use nvisy_ontology::context::Context;
pub use nvisy_ontology::entity::DetectionOutput;
pub use nvisy_ontology::record::RedactionMap;

use crate::provenance::FileAudit;

use crate::compiler::Graph;
pub use crate::compiler::{RetryPolicy, TimeoutPolicy};

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
    /// Reference-data contexts for detection.
    pub contexts: Vec<Context>,
    /// Default retry policy for graph nodes without one.
    pub default_retry: Option<RetryPolicy>,
    /// Default timeout policy for graph nodes without one.
    pub default_timeout: Option<TimeoutPolicy>,
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
    pub detection: DetectionOutput,
    /// Policy evaluation breakdown (redactions, reviews, suppressions, blocks, alerts).
    pub evaluation: PolicyEvaluation,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Per-file processing logs.
    pub file_audits: Vec<FileAudit>,
    /// Redaction mapping artifacts.
    pub redaction_maps: Vec<RedactionMap>,
    /// Per-node execution results from the DAG runner.
    pub run_output: RunOutput,
}

/// The top-level redaction engine contract.
///
/// Takes a content handler, policies, and an execution graph; returns redacted
/// output, audit records, and a full breakdown of every pipeline phase.
pub trait Engine: Send + Sync {
    /// Execute a full redaction pipeline.
    fn run(&self, input: EngineInput) -> impl Future<Output = Result<EngineOutput, Error>> + Send;
}
