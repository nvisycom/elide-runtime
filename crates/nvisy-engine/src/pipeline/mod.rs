//! Top-level engine contract, I/O types, and the [`DefaultEngine`] implementation.
//!
//! The [`Engine`] trait defines the high-level redaction pipeline contract:
//! given content identifiers and an execution graph, produce redacted
//! output together with a full audit trail and per-phase breakdown.
//!
//! [`DefaultEngine`] is the standard implementation that orchestrates the
//! detect -> evaluate -> redact pipeline and drives the DAG execution graph.

mod config;
mod default;
mod executor;
mod ontology;
mod policy;
mod runs;

use std::future::Future;

pub use config::{EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection};
pub use default::DefaultEngine;
pub use executor::{NodeOutput, RunOutput};
use nvisy_core::Error;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::entity::DetectionOutput;
use nvisy_ontology::policy::{Policies, RedactionSummary};
use nvisy_ontology::record::{PolicyEvaluation, RedactionMap};
pub use ontology::{Explainable, Explanation};
pub use runs::{NodeProgress, RunManager, RunState, RunStatus, RunSummary};
use uuid::Uuid;

use crate::compiler::Graph;
use crate::provenance::FileAudit;

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
    /// OCR subsystem configuration.
    pub ocr: Option<OcrSection>,
    /// LLM subsystem configuration.
    pub llm: Option<LlmSection>,
    /// Speech-to-text subsystem configuration.
    pub stt: Option<SttSection>,
    /// Text-to-speech subsystem configuration.
    pub tts: Option<TtsSection>,
}

/// Full result of a pipeline run.
///
/// Contains per-phase breakdown (detection, classification, policy evaluation),
/// per-source summaries, audit records, and the raw DAG execution result.
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
    pub file_audits: Vec<FileAudit>,
    /// Redaction mapping artifacts.
    pub redaction_maps: Vec<RedactionMap>,
    /// Per-node execution results from the DAG runner.
    pub run_output: RunOutput,
}

/// The top-level redaction engine contract.
///
/// Takes content identifiers and an execution graph; returns redacted
/// output, audit records, and a full breakdown of every pipeline phase.
pub trait Engine: Send + Sync {
    /// Execute a full redaction pipeline.
    fn run(&self, input: EngineInput) -> impl Future<Output = Result<EngineOutput, Error>> + Send;
}
