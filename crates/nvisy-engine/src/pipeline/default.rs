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
use super::executor::{NodeExecutor, NodeOutput, RunOutput};
use super::plan::{self, ExecutionPlan};
use super::runs::{
    EngineRuns, NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
use super::{Engine, EngineInput, EngineOutput};
use crate::graph::policy::{RetryPolicy, TimeoutPolicy};
use crate::operation::{DocumentEnvelope, SharedContext};
use crate::provenance::PolicyEvaluation;

/// Private mutable state for a single run, held inside `DefaultEngineInner`.
struct RunEntry {
    actor_id: Uuid,
    status: RunStatus,
    created_at: Timestamp,
    completed_at: Option<Timestamp>,
    nodes: Vec<NodeSnapshot>,
    cancel: CancellationToken,
}

impl RunEntry {
    fn to_snapshot(&self, id: Uuid) -> RunSnapshot {
        RunSnapshot {
            id,
            actor_id: self.actor_id,
            status: self.status,
            created_at: self.created_at,
            completed_at: self.completed_at,
            nodes: self.nodes.clone(),
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

impl Clone for DefaultEngineInner {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            default_retry: self.default_retry.clone(),
            default_timeout: self.default_timeout.clone(),
            http_client: self.http_client.clone(),
            registry: self.registry.clone(),
            runs: RwLock::new(HashMap::new()),
        }
    }
}

/// Inner state shared behind an [`Arc`].
struct DefaultEngineInner {
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
    /// All tracked runs keyed by their UUID.
    runs: RwLock<HashMap<Uuid, RunEntry>>,
}

/// Default [`Engine`] implementation.
///
/// Wraps state in an `Arc` so cloning is cheap.
#[derive(Clone)]
pub struct DefaultEngine {
    inner: Arc<DefaultEngineInner>,
}

impl std::fmt::Debug for DefaultEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultEngine")
            .field("config", &self.inner.config)
            .field("default_retry", &self.inner.default_retry)
            .field("default_timeout", &self.inner.default_timeout)
            .field("http_client", &self.inner.http_client)
            .finish()
    }
}

impl DefaultEngine {
    /// Create a new engine backed by the given registry.
    pub fn new(registry: Registry) -> Self {
        Self {
            inner: Arc::new(DefaultEngineInner {
                config: RuntimeConfig::default(),
                default_retry: None,
                default_timeout: None,
                http_client: HttpClient::default(),
                registry,
                runs: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Set the base runtime configuration.
    ///
    /// Automatically extracts default retry and timeout policies from the
    /// `[engine]` section, if present.
    pub fn with_config(mut self, config: RuntimeConfig) -> Self {
        let inner = Arc::make_mut(&mut self.inner);
        if let Some(engine) = &config.engine {
            if inner.default_retry.is_none() {
                inner.default_retry = engine.retry.clone();
            }
            if inner.default_timeout.is_none() {
                inner.default_timeout = engine.timeout.clone();
            }
        }
        inner.config = config;
        self
    }

    /// Override the default retry policy.
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        Arc::make_mut(&mut self.inner).default_retry = Some(policy);
        self
    }

    /// Override the default timeout policy.
    pub fn with_timeout(mut self, policy: TimeoutPolicy) -> Self {
        Arc::make_mut(&mut self.inner).default_timeout = Some(policy);
        self
    }

    /// Set the shared HTTP client for downstream providers.
    pub fn with_http_client(mut self, client: HttpClient) -> Self {
        Arc::make_mut(&mut self.inner).http_client = client;
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

    /// Execute a compiled [`ExecutionPlan`] by spawning concurrent tasks for
    /// each node.
    async fn run_graph(
        plan: &ExecutionPlan,
        cancel: CancellationToken,
        shared: SharedContext,
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
            let executor = NodeExecutor::new(shared.clone(), cancel.clone());

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

                let result = executor
                    .execute(&resolved, node_senders, node_receivers)
                    .await;

                if let Some(tx) = completion_tx {
                    let _ = tx.send(true);
                }

                match result {
                    Ok(output) => output,
                    Err(e) => NodeOutput {
                        node_id,
                        items_processed: 0,
                        error: Some(e.to_string()),
                        envelopes: Vec::new(),
                    },
                }
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

    /// Build [`NodeSnapshot`]s from a completed [`RunOutput`].
    fn node_snapshots(run_output: &RunOutput) -> Vec<NodeSnapshot> {
        run_output
            .node_results
            .iter()
            .map(|nr| NodeSnapshot {
                node_id: nr.node_id,
                status: if nr.error.is_none() {
                    NodeStatus::Succeeded
                } else {
                    NodeStatus::Failed
                },
                items_processed: nr.items_processed,
                error: nr.error.clone(),
            })
            .collect()
    }
}

impl Engine for DefaultEngine {
    async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = Uuid::new_v4();
        let cancel = CancellationToken::new();

        {
            let entry = RunEntry {
                actor_id: input.actor_id,
                status: RunStatus::Pending,
                created_at: Timestamp::now(),
                completed_at: None,
                nodes: Vec::new(),
                cancel: cancel.clone(),
            };
            self.inner.runs.write().await.insert(run_id, entry);
        }

        if let Some(entry) = self.inner.runs.write().await.get_mut(&run_id) {
            entry.status = RunStatus::Running;
        }

        let shared = SharedContext::new(run_id, input.actor_id)
            .with_policies(input.policies.clone())
            .with_contexts(input.contexts.clone());

        let compiled = plan::compile(
            &input.graph,
            self.inner.default_retry.as_ref(),
            self.inner.default_timeout.as_ref(),
        )?;
        let run_output = Self::run_graph(&compiled, cancel, shared).await?;

        // Transition to Succeeded/Failed and populate node snapshots
        {
            let snapshots = Self::node_snapshots(&run_output);
            if let Some(entry) = self.inner.runs.write().await.get_mut(&run_id) {
                let any_ok = run_output.node_results.iter().any(|r| r.error.is_none());
                let any_err = run_output.node_results.iter().any(|r| r.error.is_some());
                entry.status = match (any_ok, any_err) {
                    (_, false) => RunStatus::Succeeded,
                    (true, true) => RunStatus::PartialFailure,
                    _ => RunStatus::Failed,
                };
                entry.completed_at = Some(Timestamp::now());
                entry.nodes = snapshots;
            }
        }

        // Collect envelopes from all nodes (export nodes accumulate them)
        let mut all_entities = nvisy_ontology::entity::Entities::new();
        let mut file_audits = Vec::new();
        let first_policy_id = input
            .policies
            .policies
            .first()
            .map(|p| p.id)
            .unwrap_or(Uuid::nil());

        for nr in &run_output.node_results {
            for envelope in &nr.envelopes {
                all_entities.extend(envelope.entities.iter().cloned());
                file_audits.push(envelope.audit.clone());
            }
        }

        let detection =
            nvisy_ontology::entity::DetectionOutput::new(ContentSource::new(), all_entities);
        let evaluation = PolicyEvaluation::new(first_policy_id);

        Ok(EngineOutput {
            run_id,
            detection,
            evaluation,
            summaries: Vec::new(),
            file_audits,
            redaction_maps: Vec::new(),
        })
    }
}

impl EngineAnalytics for DefaultEngine {
    async fn snapshot(&self) -> AnalyticsSnapshot {
        let runs = self.inner.runs.read().await;
        AnalyticsSnapshot {
            timestamp: Timestamp::now(),
            total_runs: runs.len() as u64,
            total_entities_detected: 0,
            total_redactions_applied: 0,
        }
    }
}

impl EngineRuns for DefaultEngine {
    async fn get_run(&self, id: Uuid) -> Option<RunSnapshot> {
        self.inner
            .runs
            .read()
            .await
            .get(&id)
            .map(|entry| entry.to_snapshot(id))
    }

    async fn list_runs(&self, filter: RunFilter) -> Vec<RunSummary> {
        self.inner
            .runs
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
        let mut runs = self.inner.runs.write().await;
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
            )),
        }
    }
}
