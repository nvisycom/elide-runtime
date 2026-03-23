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
mod orchestrator;
mod plan;
mod runs;

use std::future::Future;
use std::path::Path;

use nvisy_core::Error;
use nvisy_core::content::Content;
use nvisy_ontology::context::Context as OntologyContext;
use nvisy_ontology::entity::DetectionOutput;
use nvisy_ontology::policy::{Policies, RedactionSummary};
use uuid::Uuid;

pub use self::analytics::{AnalyticsSnapshot, EngineAnalytics};
pub use self::config::{
    EngineSection, LlmSection, OcrSection, RuntimeConfig, SttSection, TtsSection,
};
pub use self::default::DefaultEngine;
pub use self::runs::{
    EngineRuns, NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
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

/// The top-level redaction engine contract.
///
/// Takes content identifiers and an execution graph; returns redacted
/// output, audit records, and a full breakdown of every pipeline phase.
pub trait Engine: Send + Sync {
    /// Execute a full redaction pipeline.
    fn run(&self, input: EngineInput) -> impl Future<Output = Result<EngineOutput, Error>> + Send;
}

/// Content and context storage operations.
///
/// Provides actor-scoped CRUD for files and contexts without
/// exposing the underlying storage backend.
pub trait EngineStorage: Send + Sync {
    /// Store content and return the assigned identifier.
    fn upload_content(
        &self,
        actor_id: Uuid,
        content: Content,
    ) -> impl Future<Output = Result<Uuid, Error>> + Send;

    /// Retrieve stored content data and metadata.
    fn download_content(
        &self,
        actor_id: Uuid,
        content_id: Uuid,
    ) -> impl Future<Output = Result<Content, Error>> + Send;

    /// List all content identifiers for an actor.
    fn list_content(
        &self,
        actor_id: Uuid,
    ) -> impl Future<Output = Result<Vec<Uuid>, Error>> + Send;

    /// Delete a single content entry.
    fn delete_content(
        &self,
        actor_id: Uuid,
        content_id: Uuid,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Delete all content for an actor. Returns the number of entries removed.
    fn delete_all_content(
        &self,
        actor_id: Uuid,
    ) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Store a context and return the assigned identifier.
    fn upload_context(
        &self,
        actor_id: Uuid,
        context: OntologyContext,
    ) -> impl Future<Output = Result<Uuid, Error>> + Send;

    /// Retrieve a stored context.
    fn download_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
    ) -> impl Future<Output = Result<OntologyContext, Error>> + Send;

    /// List all context identifiers for an actor.
    fn list_contexts(
        &self,
        actor_id: Uuid,
    ) -> impl Future<Output = Result<Vec<Uuid>, Error>> + Send;

    /// Delete a single context entry.
    fn delete_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Delete all contexts for an actor. Returns the number of entries removed.
    fn delete_all_contexts(
        &self,
        actor_id: Uuid,
    ) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Returns the base data directory path.
    fn data_dir(&self) -> &Path;
}
