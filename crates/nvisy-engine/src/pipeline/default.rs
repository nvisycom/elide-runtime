//! Engine implementation.
//!
//! [`Engine`] ties together configuration, run state, and the
//! DAG orchestrator.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::content::{Content, ContentSource};
use nvisy_http::HttpClient;
use nvisy_ontology::context::{Context, ContextMap};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::analytics::AnalyticsSnapshot;
use super::config::RuntimeConfig;
use super::orchestrator::{self, RunContext};
use super::runs::state::{RunEntry, RunState};
use super::runs::{NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary};
use super::{EngineInput, EngineOutput, plan};
use crate::graph::{GraphNodeKind, RetryPolicy, TimeoutPolicy};
use crate::operation::context::SharedContext;
use crate::operation::encryption::SharedKeyProvider;
use crate::provenance::PolicyEvaluation;
use crate::registry::Registry;

/// Immutable configuration created during engine construction.
#[derive(Clone)]
struct EngineConfig {
    config: RuntimeConfig,
    default_retry: Option<RetryPolicy>,
    default_timeout: Option<TimeoutPolicy>,
    http_client: HttpClient,
    registry: Registry,
    key_provider: Option<SharedKeyProvider>,
}

/// The redaction pipeline engine.
///
/// Immutable configuration lives in `Arc<EngineConfig>` (set during
/// construction). Mutable run state lives in [`RunState`] and is
/// shared across all clones.
#[derive(Clone)]
pub struct Engine {
    cfg: Arc<EngineConfig>,
    runs: RunState,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.cfg.config)
            .field("default_retry", &self.cfg.default_retry)
            .field("default_timeout", &self.cfg.default_timeout)
            .field("http_client", &self.cfg.http_client)
            .finish()
    }
}

impl Engine {
    /// Open a new engine backed by a registry at the given data directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry database cannot be opened.
    pub fn open(data_dir: std::path::PathBuf) -> Result<Self, Error> {
        let registry = Registry::open(data_dir)?;
        Ok(Self::new(registry))
    }

    /// Create a new engine backed by the given registry.
    pub fn new(registry: Registry) -> Self {
        Self {
            cfg: Arc::new(EngineConfig {
                config: RuntimeConfig::default(),
                default_retry: None,
                default_timeout: None,
                http_client: HttpClient::default(),
                registry,
                key_provider: None,
            }),
            runs: RunState::new(),
        }
    }

    /// Set the base runtime configuration.
    ///
    /// Automatically extracts default retry and timeout policies from the
    /// `[engine]` section, if present. Also wires `max_completed_runs`
    /// into the run state eviction policy.
    pub fn with_config(mut self, config: RuntimeConfig) -> Self {
        let cfg = Arc::make_mut(&mut self.cfg);
        if let Some(engine) = &config.engine {
            if cfg.default_retry.is_none() {
                cfg.default_retry = engine.retry.clone();
            }
            if cfg.default_timeout.is_none() {
                cfg.default_timeout = engine.timeout.clone();
            }
            if let Some(max) = engine.max_completed_runs {
                self.runs =
                    std::mem::replace(&mut self.runs, RunState::new()).with_max_completed_runs(max);
            }
        }
        cfg.config = config;
        self
    }

    /// Override the default retry policy.
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        Arc::make_mut(&mut self.cfg).default_retry = Some(policy);
        self
    }

    /// Override the default timeout policy.
    pub fn with_timeout(mut self, policy: TimeoutPolicy) -> Self {
        Arc::make_mut(&mut self.cfg).default_timeout = Some(policy);
        self
    }

    /// Set the shared HTTP client for downstream providers.
    pub fn with_http_client(mut self, client: HttpClient) -> Self {
        Arc::make_mut(&mut self.cfg).http_client = client;
        self
    }

    /// Set the key provider for encryption/decryption operations.
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        Arc::make_mut(&mut self.cfg).key_provider = Some(provider);
        self
    }

    /// Returns the base runtime configuration.
    pub fn config(&self) -> &RuntimeConfig {
        &self.cfg.config
    }

    /// Returns the shared HTTP client.
    pub fn http_client(&self) -> &HttpClient {
        &self.cfg.http_client
    }

    /// Returns the content and context registry.
    pub fn registry(&self) -> &Registry {
        &self.cfg.registry
    }

    /// Execute a full redaction pipeline.
    pub async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = uuid::Uuid::new_v4();
        let cancel = CancellationToken::new();

        let effective_config = match &input.config {
            Some(overrides) => self.cfg.config.merge(overrides),
            None => self.cfg.config.clone(),
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

        let compiled = match plan::compile(
            input.graph,
            effective_config
                .engine
                .as_ref()
                .and_then(|e| e.retry.as_ref())
                .or(self.cfg.default_retry.as_ref()),
            effective_config
                .engine
                .as_ref()
                .and_then(|e| e.timeout.as_ref())
                .or(self.cfg.default_timeout.as_ref()),
            channel_buffer,
        ) {
            Ok(plan) => plan,
            Err(e) => {
                self.runs
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

        self.runs
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
            match self.cfg.registry.read_context(input.actor_id, *id).await {
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

        let mut shared = SharedContext::new(run_id, input.actor_id, self.cfg.registry.clone())
            .with_policies(input.policies.clone())
            .with_context_map(context_map);
        if let Some(ref kp) = self.cfg.key_provider {
            shared = shared.with_key_provider(kp.clone());
        }

        let max_concurrent_nodes = effective_config
            .engine
            .as_ref()
            .and_then(|e| e.max_concurrent_nodes);
        let cancel_clone = cancel.clone();
        let ctx = RunContext {
            cancel,
            shared,
            config: effective_config,
            http_client: self.cfg.http_client.clone(),
            max_concurrent_nodes,
        };

        self.runs.set_started_at(run_id).await;

        let run_output = if let Some(ms) = run_timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(ms),
                orchestrator::run_graph(&compiled, run_id, self.runs.clone(), ctx),
            )
            .await
            {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    self.runs.fail(run_id).await;
                    return Err(e);
                }
                Err(_) => {
                    cancel_clone.cancel();
                    self.runs.fail(run_id).await;
                    return Err(Error::timeout("pipeline run exceeded time limit"));
                }
            }
        } else {
            match orchestrator::run_graph(&compiled, run_id, self.runs.clone(), ctx).await {
                Ok(output) => output,
                Err(e) => {
                    self.runs.fail(run_id).await;
                    return Err(e);
                }
            }
        };

        let (output, entities_detected, redactions_applied) =
            collect_output(run_id, &input.policies, &run_output);

        let status = run_output.run_status();
        self.runs
            .finalize(run_id, status, entities_detected, redactions_applied)
            .await;

        Ok(output)
    }

    /// Collect a point-in-time analytics snapshot.
    pub async fn snapshot(&self) -> AnalyticsSnapshot {
        self.runs.snapshot().await
    }

    /// Get a full snapshot of a single run.
    pub async fn get_run(&self, actor_id: Uuid, id: Uuid) -> Option<RunSnapshot> {
        self.runs.get_run(actor_id, id).await
    }

    /// List runs matching the given filter.
    pub async fn list_runs(&self, actor_id: Uuid, filter: RunFilter) -> Vec<RunSummary> {
        self.runs.list_runs(actor_id, &filter).await
    }

    /// Request cancellation of an in-progress run.
    ///
    /// Returns `Err` if the run was not found or has already finished.
    pub async fn cancel_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.runs.cancel_run(actor_id, id).await
    }

    /// Delete a single finished run.
    ///
    /// Returns `Err` if the run does not exist or is still active.
    pub async fn delete_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.runs.delete_run(actor_id, id).await
    }

    /// Delete all finished runs. Returns the number of removed entries.
    pub async fn delete_all_runs(&self, actor_id: Uuid) -> usize {
        self.runs.delete_all_runs(actor_id).await
    }

    /// Store content and return the assigned identifier.
    pub async fn upload_content(&self, actor_id: Uuid, content: Content) -> Result<Uuid, Error> {
        let handle = self
            .cfg
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
        let handle = self.cfg.registry.read_content(actor_id, content_id).await?;
        let data = handle.content_data().await?;
        let metadata = handle.metadata().await?;
        Ok(Content::with_metadata(data, metadata))
    }

    /// List all content identifiers for an actor.
    pub async fn list_content(&self, actor_id: Uuid) -> Result<Vec<Uuid>, Error> {
        self.cfg.registry.list_content(actor_id).await
    }

    /// Delete a single content entry.
    pub async fn delete_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<(), Error> {
        self.cfg
            .registry
            .unregister_content(actor_id, content_id)
            .await
    }

    /// Delete all content for an actor. Returns the number of entries removed.
    pub async fn delete_all_content(&self, actor_id: Uuid) -> Result<usize, Error> {
        self.cfg.registry.unregister_all_content(actor_id).await
    }

    /// Store a context and return the assigned identifier.
    pub async fn upload_context(&self, actor_id: Uuid, context: Context) -> Result<Uuid, Error> {
        let handle = self
            .cfg
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
        let handle = self.cfg.registry.read_context(actor_id, context_id).await?;
        handle.context().await
    }

    /// List all context identifiers for an actor.
    pub async fn list_contexts(&self, actor_id: Uuid) -> Result<Vec<Uuid>, Error> {
        self.cfg.registry.list_contexts(actor_id).await
    }

    /// Delete a single context entry.
    pub async fn delete_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<(), Error> {
        self.cfg
            .registry
            .unregister_context(actor_id, context_id)
            .await
    }

    /// Delete all contexts for an actor. Returns the number of entries removed.
    pub async fn delete_all_contexts(&self, actor_id: Uuid) -> Result<usize, Error> {
        self.cfg.registry.unregister_all_contexts(actor_id).await
    }

    /// Returns the base data directory path.
    pub fn data_dir(&self) -> &Path {
        self.cfg.registry.base_dir()
    }
}

/// Collect [`EngineOutput`] from completed node results.
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
