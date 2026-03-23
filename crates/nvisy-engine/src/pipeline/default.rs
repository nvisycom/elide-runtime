//! [`Engine`] implementation: the main entry point for running pipelines.
//!
//! A pipeline run follows these phases:
//!
//! 1. **Config merge** — per-request [`RuntimeConfig`] overrides are merged
//!    with the engine's base config (section-level replacement).
//! 2. **Context pre-loading** — [`LoadContext`](crate::graph::LoadContext)
//!    nodes are scanned and their context IDs are bulk-loaded from the
//!    [`Registry`] into a [`ContextMap`](nvisy_ontology::context::ContextMap)
//!    before execution begins.
//! 3. **Compile** — the [`Graph`] is compiled into an
//!    [`ExecutionPlan`](super::plan::ExecutionPlan) via [`plan::compile`].
//! 4. **Schedule & execute** — the orchestrator spawns concurrent tasks
//!    per node and collects results (see [`orchestrator::run_graph`]).
//! 5. **Finalize** — detection counts and redaction counts are computed
//!    from node results, the run status is recorded, and an
//!    [`EngineOutput`] is returned.
//!
//! All mutable state lives in [`EngineInner`] behind an `Arc`. Builder
//! methods (`with_key_provider`) require exclusive access via
//! `Arc::get_mut` and must be called before the engine is cloned.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::content::{Content, ContentSource};
use nvisy_http::HttpClient;
use nvisy_ontology::context::{Context, ContextMap};
use nvisy_ontology::entity::DetectionOutput;
use nvisy_ontology::policy::{Policies, RedactionSummary};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::analytics::AnalyticsSnapshot;
use super::config::RuntimeConfig;
use super::orchestrator::{self, RunContext};
use super::plan;
use super::runs::state::{RunEntry, RunState};
use super::runs::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
use crate::graph::{Graph, GraphNodeKind, RetryPolicy, TimeoutPolicy};
use crate::operation::context::SharedContext;
use crate::operation::encryption::SharedKeyProvider;
use crate::provenance::{Audit, PolicyEvaluation, RedactionMap};
use crate::registry::Registry;

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

/// Shared inner state for the engine, behind an `Arc`.
///
/// All fields are immutable after construction except [`RunState`],
/// which is internally synchronized.
struct EngineInner {
    /// Base configuration (merged with per-request overrides at runtime).
    config: RuntimeConfig,
    /// Engine-level default retry policy, applied to nodes that lack one.
    default_retry: Option<RetryPolicy>,
    /// Engine-level default timeout policy, applied to nodes that lack one.
    default_timeout: Option<TimeoutPolicy>,
    /// Shared HTTP client for all downstream API calls.
    http_client: HttpClient,
    /// Content and context storage backend.
    registry: Registry,
    /// Optional encryption key provider for encrypt/decrypt operations.
    key_provider: Option<SharedKeyProvider>,
    /// In-memory run lifecycle tracker.
    runs: RunState,
}

/// The redaction pipeline engine.
///
/// All state lives in `Arc<EngineInner>` and is shared across clones.
/// Builder methods (`with_config`, `with_retry`, etc.) use
/// `Arc::get_mut` and must be called before the engine is cloned.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.inner.config)
            .field("default_retry", &self.inner.default_retry)
            .field("default_timeout", &self.inner.default_timeout)
            .field("http_client", &self.inner.http_client)
            .finish()
    }
}

impl Engine {
    /// Open a new engine with the given configuration and data directory.
    ///
    /// Constructs the HTTP client, registry, and run state from the
    /// provided config. Extracts default retry/timeout policies and
    /// run eviction limits from the `[engine]` section, if present.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub fn open(data_dir: impl AsRef<Path>, config: RuntimeConfig) -> Result<Self, Error> {
        let registry = Registry::open(data_dir.as_ref())?;

        let http_config = config
            .engine
            .as_ref()
            .and_then(|e| e.http.clone())
            .unwrap_or_default();
        let http_client = HttpClient::new(&http_config);

        let mut default_retry = None;
        let mut default_timeout = None;

        if let Some(engine) = &config.engine {
            default_retry = engine.retry.clone();
            default_timeout = engine.timeout.clone();
        }

        Ok(Self {
            inner: Arc::new(EngineInner {
                config,
                default_retry,
                default_timeout,
                http_client,
                registry,
                key_provider: None,
                runs: RunState::new(),
            }),
        })
    }

    /// Create a temporary engine backed by a [`tempfile`] directory.
    ///
    /// Uses default configuration. The directory is deleted when the
    /// returned [`TempDir`](tempfile::TempDir) is dropped — keep it
    /// alive for the duration of the test.
    ///
    /// # Panics
    ///
    /// Panics if the temp directory or registry cannot be created.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn temp() -> (Self, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("failed to create temp directory");
        let engine = Self::open(dir.path(), RuntimeConfig::default())
            .expect("failed to open engine in temp directory");
        (engine, dir)
    }

    /// Set the key provider for encryption/decryption operations.
    ///
    /// # Panics
    ///
    /// Panics if the engine has already been cloned (Arc is shared).
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        Arc::get_mut(&mut self.inner)
            .expect("engine must not be shared during construction")
            .key_provider = Some(provider);
        self
    }

    /// Returns the base runtime configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.inner.config
    }

    /// Returns the shared HTTP client.
    pub fn http_client(&self) -> &HttpClient {
        &self.inner.http_client
    }

    /// Returns the content and context registry.
    pub fn registry(&self) -> &Registry {
        &self.inner.registry
    }

    /// Execute a full redaction pipeline.
    ///
    /// Compiles the graph, pre-loads contexts, runs the DAG orchestrator,
    /// and collects results. See the [module docs](super::default) for the
    /// full phase breakdown.
    ///
    /// # Errors
    ///
    /// Returns an error if graph compilation fails, the run times out, or
    /// all nodes fail during execution.
    pub async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = uuid::Uuid::new_v4();
        let cancel = CancellationToken::new();

        let effective_config = match &input.config {
            Some(overrides) => self.inner.config.merge(overrides),
            None => self.inner.config.clone(),
        };

        let mut context_ids: Vec<Uuid> = input
            .graph
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                GraphNodeKind::LoadContext(cfg) => Some(cfg.context_ids.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        context_ids.sort_unstable();
        context_ids.dedup();

        let channel_buffer = effective_config
            .engine
            .as_ref()
            .and_then(|e| e.channel_buffer);
        let run_timeout_ms = effective_config
            .engine
            .as_ref()
            .and_then(|e| e.run_timeout_ms);
        let concurrency = input
            .graph
            .concurrency
            .or_else(|| effective_config.engine.as_ref().and_then(|e| e.concurrency));

        let compiled = match plan::compile(
            input.graph,
            effective_config
                .engine
                .as_ref()
                .and_then(|e| e.retry.as_ref())
                .or(self.inner.default_retry.as_ref()),
            effective_config
                .engine
                .as_ref()
                .and_then(|e| e.timeout.as_ref())
                .or(self.inner.default_timeout.as_ref()),
            channel_buffer,
        ) {
            Ok(plan) => plan,
            Err(e) => {
                self.inner
                    .runs
                    .insert(
                        run_id,
                        RunEntry {
                            actor_id: input.actor_id,
                            status: RunStatus::Failed,
                            created_at: jiff::Timestamp::now(),
                            started_at: None,
                            completed_at: Some(jiff::Timestamp::now()),
                            nodes: HashMap::new(),
                            cancel: cancel.clone(),
                            entities_detected: 0,
                            redactions_applied: 0,
                        },
                    )
                    .await;
                return Err(e);
            }
        };

        let initial_nodes: HashMap<Uuid, NodeSnapshot> = compiled
            .nodes()
            .iter()
            .map(|n| {
                (
                    n.node.id,
                    NodeSnapshot {
                        node_id: n.node.id,
                        status: NodeStatus::Pending,
                        items_processed: 0,
                        error: None,
                    },
                )
            })
            .collect();

        self.inner
            .runs
            .insert(
                run_id,
                RunEntry {
                    actor_id: input.actor_id,
                    status: RunStatus::Running,
                    created_at: jiff::Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    nodes: initial_nodes,
                    cancel: cancel.clone(),
                    entities_detected: 0,
                    redactions_applied: 0,
                },
            )
            .await;

        let mut context_map = ContextMap::new();
        let mut read_failures = 0u64;
        let mut deserialize_failures = 0u64;
        for id in &context_ids {
            match self.inner.registry.read_context(input.actor_id, *id).await {
                Ok(handle) => match handle.context().await {
                    Ok(context) => {
                        context_map.insert(context);
                    }
                    Err(e) => {
                        deserialize_failures += 1;
                        tracing::warn!(%id, error = %e, "failed to deserialize context, skipping");
                    }
                },
                Err(e) => {
                    read_failures += 1;
                    tracing::warn!(%id, error = %e, "failed to read context, skipping");
                }
            }
        }
        let total_failures = read_failures + deserialize_failures;
        if total_failures > 0 {
            tracing::info!(
                read_failures,
                deserialize_failures,
                "context loading completed with {total_failures} failure(s)"
            );
        }
        tracing::debug!(loaded = context_map.len(), "pre-loaded contexts");

        let mut shared = SharedContext::new(run_id, input.actor_id, self.inner.registry.clone())
            .with_policies(input.policies.clone())
            .with_context_map(context_map);
        if let Some(ref kp) = self.inner.key_provider {
            shared = shared.with_key_provider(kp.clone());
        }

        let cancel_clone = cancel.clone();
        let ctx = RunContext {
            cancel,
            shared,
            config: effective_config,
            http_client: self.inner.http_client.clone(),
            concurrency,
        };

        self.inner.runs.set_started_at(run_id).await;

        let run_output = if let Some(ms) = run_timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(ms),
                orchestrator::run_graph(&compiled, run_id, self.inner.runs.clone(), ctx),
            )
            .await
            {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    self.inner.runs.fail(run_id).await;
                    return Err(e);
                }
                Err(_) => {
                    cancel_clone.cancel();
                    self.inner.runs.fail(run_id).await;
                    return Err(Error::timeout("pipeline run exceeded time limit"));
                }
            }
        } else {
            match orchestrator::run_graph(&compiled, run_id, self.inner.runs.clone(), ctx).await {
                Ok(output) => output,
                Err(e) => {
                    self.inner.runs.fail(run_id).await;
                    return Err(e);
                }
            }
        };

        let (output, entities_detected, redactions_applied) =
            collect_output(run_id, &input.policies, &run_output);

        let status = run_output.run_status();
        self.inner
            .runs
            .finalize(run_id, status, entities_detected, redactions_applied)
            .await;

        Ok(output)
    }

    /// Collect a point-in-time analytics snapshot.
    pub async fn snapshot(&self) -> AnalyticsSnapshot {
        self.inner.runs.snapshot().await
    }

    /// Get a full snapshot of a single run.
    pub async fn get_run(&self, actor_id: Uuid, id: Uuid) -> Option<RunSnapshot> {
        self.inner.runs.get_run(actor_id, id).await
    }

    /// List runs matching the given filter.
    pub async fn list_runs(&self, actor_id: Uuid, filter: RunFilter) -> Vec<RunSummary> {
        self.inner.runs.list_runs(actor_id, &filter).await
    }

    /// Request cancellation of an in-progress run.
    ///
    /// Returns `Err` if the run was not found or has already finished.
    pub async fn cancel_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.runs.cancel_run(actor_id, id).await
    }

    /// Delete a single finished run.
    ///
    /// Returns `Err` if the run does not exist or is still active.
    pub async fn delete_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.inner.runs.delete_run(actor_id, id).await
    }

    /// Delete all finished runs. Returns the number of removed entries.
    pub async fn delete_all_runs(&self, actor_id: Uuid) -> usize {
        self.inner.runs.delete_all_runs(actor_id).await
    }

    /// Store content and return the assigned identifier.
    pub async fn upload_content(&self, actor_id: Uuid, content: Content) -> Result<Uuid, Error> {
        let handle = self
            .inner
            .registry
            .register_content(actor_id, content)
            .await?;
        Ok(handle.content_source().as_uuid())
    }

    /// Retrieve stored content data and metadata.
    pub async fn download_content(
        &self,
        actor_id: Uuid,
        content_id: Uuid,
    ) -> Result<Content, Error> {
        let handle = self
            .inner
            .registry
            .read_content(actor_id, content_id)
            .await?;
        let data = handle.content_data().await?;
        let metadata = handle.metadata().await?;
        Ok(Content::with_metadata(data, metadata))
    }

    /// List all content identifiers for an actor.
    pub async fn list_content(&self, actor_id: Uuid) -> Result<Vec<Uuid>, Error> {
        self.inner.registry.list_content(actor_id).await
    }

    /// Delete a single content entry.
    pub async fn delete_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<(), Error> {
        self.inner
            .registry
            .unregister_content(actor_id, content_id)
            .await
    }

    /// Delete all content for an actor. Returns the number of entries removed.
    pub async fn delete_all_content(&self, actor_id: Uuid) -> Result<usize, Error> {
        self.inner.registry.unregister_all_content(actor_id).await
    }

    /// Store a context and return the assigned identifier.
    pub async fn upload_context(&self, actor_id: Uuid, context: Context) -> Result<Uuid, Error> {
        let handle = self
            .inner
            .registry
            .register_context(actor_id, context)
            .await?;
        Ok(handle.source().as_uuid())
    }

    /// Retrieve a stored context.
    pub async fn download_context(
        &self,
        actor_id: Uuid,
        context_id: Uuid,
    ) -> Result<Context, Error> {
        let handle = self
            .inner
            .registry
            .read_context(actor_id, context_id)
            .await?;
        handle.context().await
    }

    /// List all context identifiers for an actor.
    pub async fn list_contexts(&self, actor_id: Uuid) -> Result<Vec<Uuid>, Error> {
        self.inner.registry.list_contexts(actor_id).await
    }

    /// Delete a single context entry.
    pub async fn delete_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<(), Error> {
        self.inner
            .registry
            .unregister_context(actor_id, context_id)
            .await
    }

    /// Delete all contexts for an actor. Returns the number of entries removed.
    pub async fn delete_all_contexts(&self, actor_id: Uuid) -> Result<usize, Error> {
        self.inner.registry.unregister_all_contexts(actor_id).await
    }

    /// Returns the base data directory path.
    pub fn data_dir(&self) -> &Path {
        self.inner.registry.base_dir()
    }
}

/// Aggregate entity detections, policy decisions, and audit records from
/// all completed node results into a single [`EngineOutput`].
///
/// Returns `(output, entities_detected, redactions_applied)` for
/// finalization bookkeeping.
fn collect_output(
    run_id: Uuid,
    policies: &nvisy_ontology::policy::Policies,
    run_output: &super::executor::RunOutput,
) -> (EngineOutput, u64, u64) {
    let mut all_entities = nvisy_ontology::entity::Entities::new();
    let mut all_decisions = Vec::new();
    let mut all_records = Vec::new();
    let mut file_audits = Vec::new();
    let mut redactions_applied = 0u64;

    for nr in &run_output.node_results {
        for envelope in &nr.envelopes {
            all_entities.extend(envelope.entities.iter().cloned());
            all_decisions.extend(envelope.audit.decisions.iter().cloned());
            all_records.extend(envelope.audit.records.iter().cloned());
            redactions_applied += envelope
                .audit
                .decisions
                .iter()
                .filter(|d| d.applied)
                .count() as u64;
            file_audits.push(envelope.audit.clone());
        }
    }

    let entities_detected = all_entities.len() as u64;

    let policy_id = policies
        .policies
        .first()
        .map(|p| p.id)
        .unwrap_or(Uuid::nil());

    let mut evaluation = PolicyEvaluation::new(policy_id);
    evaluation.decisions = all_decisions;
    evaluation.records = all_records;

    let output = EngineOutput {
        run_id,
        detection: nvisy_ontology::entity::DetectionOutput::new(ContentSource::new(), all_entities),
        evaluation,
        summaries: Vec::new(),
        file_audits,
        redaction_maps: Vec::new(),
    };

    (output, entities_detected, redactions_applied)
}
