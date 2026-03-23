//! Default engine implementation.
//!
//! [`DefaultEngine`] ties together configuration, run state, and the
//! DAG orchestrator. It implements [`Engine`], [`EngineAnalytics`], and
//! [`EngineRuns`].

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::content::{Content, ContentSource};
use nvisy_http::HttpClient;
use nvisy_ontology::context::{Context, ContextMap};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::analytics::{AnalyticsSnapshot, EngineAnalytics};
use super::config::RuntimeConfig;
use super::orchestrator::{self, RunContext};
use super::runs::state::{RunEntry, RunState};
use super::runs::{
    EngineRuns, NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
use super::{Engine, EngineInput, EngineOutput, EngineStorage, plan};
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

/// Default [`Engine`] implementation.
///
/// Immutable configuration lives in `Arc<EngineConfig>` (set during
/// construction). Mutable run state lives in [`RunState`] and is
/// shared across all clones.
#[derive(Clone)]
pub struct DefaultEngine {
    cfg: Arc<EngineConfig>,
    runs: RunState,
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
}

impl Engine for DefaultEngine {
    async fn run(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let run_id = uuid::Uuid::new_v4();
        let cancel = CancellationToken::new();

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
                self.runs
                    .insert(
                        run_id,
                        RunEntry {
                            actor_id: input.actor_id,
                            status: RunStatus::Failed,
                            created_at: jiff::Timestamp::now(),
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
                    completed_at: None,
                    nodes: initial_nodes,
                    cancel: cancel.clone(),
                    entities_detected: 0,
                    redactions_applied: 0,
                },
            )
            .await;

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

        let mut context_map = ContextMap::new();
        for id in &context_ids {
            match self.cfg.registry.read_context(input.actor_id, *id).await {
                Ok(handle) => match handle.context().await {
                    Ok(context) => {
                        context_map.insert(context);
                    }
                    Err(e) => {
                        tracing::warn!(%id, error = %e, "failed to load context, skipping");
                    }
                },
                Err(e) => {
                    tracing::warn!(%id, error = %e, "failed to load context, skipping");
                }
            }
        }
        tracing::debug!(loaded = context_map.len(), "pre-loaded contexts");

        let mut shared = SharedContext::new(run_id, input.actor_id, self.cfg.registry.clone())
            .with_policies(input.policies.clone())
            .with_context_map(context_map);
        if let Some(ref kp) = self.cfg.key_provider {
            shared = shared.with_key_provider(kp.clone());
        }

        let ctx = RunContext {
            cancel,
            shared,
            config: effective_config,
            http_client: self.cfg.http_client.clone(),
        };

        let run_output =
            match orchestrator::run_graph(&compiled, run_id, self.runs.clone(), ctx).await {
                Ok(output) => output,
                Err(e) => {
                    self.runs.fail(run_id).await;
                    return Err(e);
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

impl EngineAnalytics for DefaultEngine {
    async fn snapshot(&self) -> AnalyticsSnapshot {
        self.runs.snapshot().await
    }
}

impl EngineRuns for DefaultEngine {
    async fn get_run(&self, actor_id: Uuid, id: Uuid) -> Option<RunSnapshot> {
        self.runs.get_run(actor_id, id).await
    }

    async fn list_runs(&self, actor_id: Uuid, filter: RunFilter) -> Vec<RunSummary> {
        self.runs.list_runs(actor_id, &filter).await
    }

    async fn cancel_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.runs.cancel_run(actor_id, id).await
    }

    async fn delete_run(&self, actor_id: Uuid, id: Uuid) -> Result<(), Error> {
        self.runs.delete_run(actor_id, id).await
    }

    async fn delete_all_runs(&self, actor_id: Uuid) -> usize {
        self.runs.delete_all_runs(actor_id).await
    }
}

impl EngineStorage for DefaultEngine {
    async fn upload_content(&self, actor_id: Uuid, content: Content) -> Result<Uuid, Error> {
        let handle = self
            .cfg
            .registry
            .register_content(actor_id, content)
            .await?;
        Ok(handle.content_source().as_uuid())
    }

    async fn download_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<Content, Error> {
        let handle = self.cfg.registry.read_content(actor_id, content_id).await?;
        let data = handle.content_data().await?;
        let metadata = handle.metadata().await?;
        Ok(Content::with_metadata(data, metadata))
    }

    async fn list_content(&self, actor_id: Uuid) -> Result<Vec<Uuid>, Error> {
        self.cfg.registry.list_content(actor_id).await
    }

    async fn delete_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<(), Error> {
        self.cfg
            .registry
            .unregister_content(actor_id, content_id)
            .await
    }

    async fn delete_all_content(&self, actor_id: Uuid) -> Result<usize, Error> {
        self.cfg.registry.unregister_all_content(actor_id).await
    }

    async fn upload_context(&self, actor_id: Uuid, context: Context) -> Result<Uuid, Error> {
        let handle = self
            .cfg
            .registry
            .register_context(actor_id, context)
            .await?;
        Ok(handle.source().as_uuid())
    }

    async fn download_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<Context, Error> {
        let handle = self.cfg.registry.read_context(actor_id, context_id).await?;
        handle.context().await
    }

    async fn list_contexts(&self, actor_id: Uuid) -> Result<Vec<Uuid>, Error> {
        self.cfg.registry.list_contexts(actor_id).await
    }

    async fn delete_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<(), Error> {
        self.cfg
            .registry
            .unregister_context(actor_id, context_id)
            .await
    }

    async fn delete_all_contexts(&self, actor_id: Uuid) -> Result<usize, Error> {
        self.cfg.registry.unregister_all_contexts(actor_id).await
    }

    fn data_dir(&self) -> &std::path::Path {
        self.cfg.registry.base_dir()
    }
}
