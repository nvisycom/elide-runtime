//! Default engine implementation that orchestrates the full pipeline.
//!
//! [`DefaultEngine`] executes the three-phase pipeline:
//!
//! 1. **Detect**: run configured detection methods on the input content.
//! 2. **Evaluate**: map detected entities to redaction instructions via policies.
//! 3. **Redact**: apply redaction instructions to produce output content.
//!
//! After the content-level pipeline completes, the execution graph is run
//! so that any Source/Action/Target DAG nodes are also executed.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use nvisy_core::Error;
use nvisy_core::content::ContentSource;
use nvisy_http::HttpClient;
use nvisy_registry::Registry;
use tokio::sync::{RwLock, mpsc, watch};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::analytics::{AnalyticsSnapshot, EngineAnalytics};
use super::config::RuntimeConfig;
use super::executor::{ImportSource, NodeExecutor, NodeOutput, RunOutput};
use super::plan::{self, ExecutionPlan};
use super::runs::{
    EngineRuns, NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
use super::{Engine, EngineInput, EngineOutput};
use crate::graph::GraphNodeKind;
use crate::graph::policy::{RetryPolicy, TimeoutPolicy};
use crate::operation::{DocumentEnvelope, SharedContext};
use crate::provenance::PolicyEvaluation;

/// Shared handle to the runs map, passed into spawned node tasks so they
/// can update their own [`NodeSnapshot`] in real time.
type RunMap = Arc<RwLock<HashMap<Uuid, RunEntry>>>;

/// Private mutable state for a single run.
struct RunEntry {
    actor_id: Uuid,
    status: RunStatus,
    created_at: Timestamp,
    completed_at: Option<Timestamp>,
    nodes: HashMap<Uuid, NodeSnapshot>,
    cancel: CancellationToken,
    entities_detected: u64,
    redactions_applied: u64,
}

impl RunEntry {
    fn to_snapshot(&self, id: Uuid) -> RunSnapshot {
        RunSnapshot {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            nodes: self.nodes.values().cloned().collect(),
        }
    }

    fn to_summary(&self, id: Uuid) -> RunSummary {
        RunSummary {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            node_count: self.nodes.len(),
        }
    }
}

/// Immutable configuration created during engine construction.
///
/// Separated from run state so that `Clone` is straightforward and
/// builder methods (`with_config`, etc.) cannot accidentally be called
/// after runs have been created.
#[derive(Clone)]
struct EngineConfig {
    /// Base runtime configuration (OCR, LLM, STT, TTS sections).
    config: RuntimeConfig,
    /// Default retry policy for graph nodes.
    default_retry: Option<RetryPolicy>,
    /// Default timeout policy for graph nodes.
    default_timeout: Option<TimeoutPolicy>,
    /// Shared HTTP client for downstream providers.
    http_client: HttpClient,
    /// Content and context storage.
    registry: Registry,
}

/// Default [`Engine`] implementation.
///
/// Immutable configuration lives in `Arc<EngineConfig>` (set during
/// construction). Mutable run state lives in `Arc<RwLock<...>>` and is
/// shared across all clones of the same engine instance.
#[derive(Clone)]
pub struct DefaultEngine {
    cfg: Arc<EngineConfig>,
    runs: Arc<RwLock<HashMap<Uuid, RunEntry>>>,
}

impl std::fmt::Debug for DefaultEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultEngine")
            .field("config", &self.cfg.config)
            .field("default_retry", &self.cfg.default_retry)
            .field("default_timeout", &self.cfg.default_timeout)
            .field("http_client", &self.cfg.http_client)
            .finish()
    }
}

impl DefaultEngine {
    /// Create a new engine backed by the given registry.
    pub fn new(registry: Registry) -> Self {
        Self {
            cfg: Arc::new(EngineConfig {
                config: RuntimeConfig::default(),
                default_retry: None,
                default_timeout: None,
                http_client: HttpClient::default(),
                registry,
            }),
            runs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set the base runtime configuration.
    ///
    /// Automatically extracts default retry and timeout policies from the
    /// `[engine]` section, if present.
    pub fn with_config(mut self, config: RuntimeConfig) -> Self {
        let cfg = Arc::make_mut(&mut self.cfg);
        if let Some(engine) = &config.engine {
            if cfg.default_retry.is_none() {
                cfg.default_retry = engine.retry.clone();
            }
            if cfg.default_timeout.is_none() {
                cfg.default_timeout = engine.timeout.clone();
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

    /// Execute a compiled [`ExecutionPlan`] by spawning concurrent tasks for
    /// each node. Each task updates its [`NodeSnapshot`] in the shared
    /// `runs` map so that `GET /runs/{id}` reflects live progress.
    #[allow(clippy::too_many_arguments)]
    async fn run_graph(
        plan: &ExecutionPlan,
        run_id: Uuid,
        runs: RunMap,
        cancel: CancellationToken,
        shared: SharedContext,
        import_source: ImportSource,
        config: RuntimeConfig,
        http_client: HttpClient,
    ) -> Result<RunOutput, Error> {
        let mut senders: HashMap<Uuid, Vec<mpsc::Sender<Arc<DocumentEnvelope>>>> = HashMap::new();
        let mut receivers: HashMap<Uuid, Vec<mpsc::Receiver<Arc<DocumentEnvelope>>>> =
            HashMap::new();

        for edge in plan.edges() {
            let (tx, rx) = mpsc::channel(edge.config.channel_buffer);
            senders.entry(edge.source).or_default().push(tx);
            receivers.entry(edge.target).or_default().push(rx);
        }

        let mut signal_senders: HashMap<Uuid, watch::Sender<bool>> = HashMap::new();
        let mut signal_receivers: HashMap<Uuid, watch::Receiver<bool>> = HashMap::new();

        for resolved in plan.nodes() {
            let (tx, rx) = watch::channel(false);
            signal_senders.insert(resolved.node.id, tx);
            signal_receivers.insert(resolved.node.id, rx);
        }

        let mut join_set: JoinSet<NodeOutput> = JoinSet::new();

        for resolved in plan.nodes() {
            let resolved = resolved.clone();
            let node_id = resolved.node.id;
            let runs = runs.clone();

            let mut executor = NodeExecutor::new(
                shared.clone(),
                cancel.clone(),
                config.clone(),
                http_client.clone(),
            );
            if matches!(resolved.node.kind, GraphNodeKind::Import(_)) {
                executor = executor.with_import_source(import_source.clone());
            }

            let upstream_watches: Vec<watch::Receiver<bool>> = resolved
                .upstream_ids
                .iter()
                .filter_map(|id| signal_receivers.get(id).cloned())
                .collect();

            let completion_tx = signal_senders.remove(&node_id);
            let node_senders = senders.remove(&node_id).unwrap_or_default();
            let node_receivers = receivers.remove(&node_id).unwrap_or_default();

            join_set.spawn(async move {
                for mut rx in upstream_watches {
                    let _ = rx.wait_for(|&done| done).await;
                }

                Self::update_node(&runs, run_id, node_id, NodeStatus::Running, 0, None)
                    .await;

                let result = executor
                    .execute(&resolved, node_senders, node_receivers)
                    .await;

                if let Some(tx) = completion_tx {
                    let _ = tx.send(true);
                }

                let output = match result {
                    Ok(output) => output,
                    Err(e) => NodeOutput {
                        node_id,
                        items_processed: 0,
                        error: Some(e.to_string()),
                        envelopes: Vec::new(),
                    },
                };

                let status = if output.error.is_none() {
                    NodeStatus::Succeeded
                } else {
                    NodeStatus::Failed
                };
                Self::update_node(
                    &runs,
                    run_id,
                    node_id,
                    status,
                    output.items_processed,
                    output.error.clone(),
                )
                .await;

                output
            });
        }

        let mut node_results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(nr) => node_results.push(nr),
                Err(e) => node_results.push(NodeOutput {
                    node_id: Uuid::nil(),
                    items_processed: 0,
                    error: Some(format!("Task panicked: {}", e)),
                    envelopes: Vec::new(),
                }),
            }
        }

        Ok(RunOutput { node_results })
    }

    /// Update a single node's snapshot within a run.
    async fn update_node(
        runs: &RunMap,
        run_id: Uuid,
        node_id: Uuid,
        status: NodeStatus,
        items_processed: u64,
        error: Option<String>,
    ) {
        if let Some(entry) = runs.write().await.get_mut(&run_id)
            && let Some(node) = entry.nodes.get_mut(&node_id)
        {
            node.status = status;
            node.items_processed = items_processed;
            node.error = error;
        }
    }

    /// Determine overall run status from node results.
    fn run_status(run_output: &RunOutput) -> RunStatus {
        let any_ok = run_output.node_results.iter().any(|r| r.error.is_none());
        let any_err = run_output.node_results.iter().any(|r| r.error.is_some());
        match (any_ok, any_err) {
            (_, false) => RunStatus::Succeeded,
            (true, true) => RunStatus::PartialFailure,
            _ => RunStatus::Failed,
        }
    }

    /// Transition a run to its final status.
    async fn finalize_run(&self, run_id: Uuid, status: RunStatus) {
        if let Some(entry) = self.runs.write().await.get_mut(&run_id) {
            entry.status = status;
            entry.completed_at = Some(Timestamp::now());
        }
    }
}

impl Engine for DefaultEngine {
    async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = Uuid::new_v4();
        let cancel = CancellationToken::new();

        // Merge per-request config overrides with engine defaults.
        let effective_config = match &input.config {
            Some(overrides) => self.cfg.config.merge(overrides),
            None => self.cfg.config.clone(),
        };

        let compiled = match plan::compile(
            &input.graph,
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
        ) {
            Ok(plan) => plan,
            Err(e) => {
                self.runs.write().await.insert(
                    run_id,
                    RunEntry {
                        actor_id: input.actor_id,
                        status: RunStatus::Failed,
                        created_at: Timestamp::now(),
                        completed_at: Some(Timestamp::now()),
                        nodes: HashMap::new(),
                        cancel: cancel.clone(),
                        entities_detected: 0,
                        redactions_applied: 0,
                    },
                );
                return Err(e);
            }
        };

        // Seed all nodes as Pending so GET /runs/{id} shows them immediately.
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

        self.runs.write().await.insert(
            run_id,
            RunEntry {
                actor_id: input.actor_id,
                status: RunStatus::Running,
                created_at: Timestamp::now(),
                completed_at: None,
                nodes: initial_nodes,
                cancel: cancel.clone(),
                entities_detected: 0,
                redactions_applied: 0,
            },
        );

        let shared = SharedContext::new(run_id, input.actor_id)
            .with_policies(input.policies.clone())
            .with_contexts(input.contexts.clone());

        let import_source = ImportSource {
            registry: self.cfg.registry.clone(),
            content_ids: input.content_ids.into(),
        };

        let run_output = match Self::run_graph(
            &compiled,
            run_id,
            self.runs.clone(),
            cancel,
            shared,
            import_source,
            effective_config,
            self.cfg.http_client.clone(),
        )
        .await
        {
            Ok(output) => output,
            Err(e) => {
                self.finalize_run(run_id, RunStatus::Failed).await;
                return Err(e);
            }
        };

        // Collect envelopes from export nodes (they accumulate downstream results).
        let mut all_entities = nvisy_ontology::entity::Entities::new();
        let mut file_audits = Vec::new();
        let mut redactions_applied = 0u64;

        for nr in &run_output.node_results {
            for envelope in &nr.envelopes {
                all_entities.extend(envelope.entities.iter().cloned());
                redactions_applied += envelope.audit.decisions.iter()
                    .filter(|d| d.applied)
                    .count() as u64;
                file_audits.push(envelope.audit.clone());
            }
        }

        let entities_detected = all_entities.len() as u64;

        let status = Self::run_status(&run_output);
        {
            let mut runs = self.runs.write().await;
            if let Some(entry) = runs.get_mut(&run_id) {
                entry.status = status;
                entry.completed_at = Some(Timestamp::now());
                entry.entities_detected = entities_detected;
                entry.redactions_applied = redactions_applied;
            }
        }

        let policy_id = input
            .policies
            .policies
            .first()
            .map(|p| p.id)
            .unwrap_or(Uuid::nil());

        Ok(EngineOutput {
            run_id,
            detection: nvisy_ontology::entity::DetectionOutput::new(
                ContentSource::new(),
                all_entities,
            ),
            evaluation: PolicyEvaluation::new(policy_id),
            summaries: Vec::new(),
            file_audits,
            redaction_maps: Vec::new(),
        })
    }
}

impl EngineAnalytics for DefaultEngine {
    async fn snapshot(&self) -> AnalyticsSnapshot {
        let runs = self.runs.read().await;
        let mut active = 0u64;
        let mut succeeded = 0u64;
        let mut failed = 0u64;
        let mut cancelled = 0u64;
        let mut total_entities = 0u64;
        let mut total_redactions = 0u64;
        let mut actors = std::collections::HashSet::new();

        for entry in runs.values() {
            match entry.status {
                RunStatus::Pending | RunStatus::Running => active += 1,
                RunStatus::Succeeded => succeeded += 1,
                RunStatus::Failed | RunStatus::PartialFailure => failed += 1,
                RunStatus::Cancelled => cancelled += 1,
            }
            actors.insert(entry.actor_id);
            total_entities += entry.entities_detected;
            total_redactions += entry.redactions_applied;
        }

        AnalyticsSnapshot {
            timestamp: Timestamp::now(),
            total_runs: runs.len() as u64,
            active_runs: active,
            succeeded_runs: succeeded,
            failed_runs: failed,
            cancelled_runs: cancelled,
            total_entities_detected: total_entities,
            total_redactions_applied: total_redactions,
            distinct_actors: actors.len() as u64,
        }
    }
}

impl EngineRuns for DefaultEngine {
    async fn get_run(&self, id: Uuid) -> Option<RunSnapshot> {
        self.runs
            .read()
            .await
            .get(&id)
            .map(|entry| entry.to_snapshot(id))
    }

    async fn list_runs(&self, filter: RunFilter) -> Vec<RunSummary> {
        self.runs
            .read()
            .await
            .iter()
            .filter(|(_, entry)| {
                filter.status.is_none_or(|s| entry.status == s)
                    && filter.actor_id.is_none_or(|a| entry.actor_id == a)
            })
            .map(|(&id, entry)| entry.to_summary(id))
            .collect()
    }

    async fn cancel_run(&self, id: Uuid) -> Result<(), Error> {
        let mut runs = self.runs.write().await;
        let entry = runs
            .get_mut(&id)
            .ok_or_else(|| Error::new(nvisy_core::ErrorKind::NotFound, "run not found"))?;

        match entry.status {
            RunStatus::Pending | RunStatus::Running => {
                entry.cancel.cancel();
                entry.status = RunStatus::Cancelled;
                entry.completed_at = Some(Timestamp::now());
                Ok(())
            }
            _ => Err(Error::new(
                nvisy_core::ErrorKind::Validation,
                "run has already finished",
            ).with_component("run")),
        }
    }
}
