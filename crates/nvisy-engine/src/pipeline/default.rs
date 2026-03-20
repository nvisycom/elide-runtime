//! Default engine implementation.
//!
//! [`DefaultEngine`] ties together configuration, run state, and the
//! DAG orchestrator. It implements [`Engine`], [`EngineAnalytics`], and
//! [`EngineRuns`].

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_core::content::ContentSource;
use nvisy_http::HttpClient;
use nvisy_registry::Registry;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::analytics::{AnalyticsSnapshot, EngineAnalytics};
use super::config::RuntimeConfig;
use super::orchestrator::{self, RunContext};
use super::runs::state::{RunEntry, RunState};
use super::runs::{
    EngineRuns, NodeSnapshot, NodeStatus, RunFilter, RunSnapshot, RunStatus, RunSummary,
};
use super::{Engine, EngineInput, EngineOutput, plan};
use crate::graph::{RetryPolicy, TimeoutPolicy};
use crate::operation::context::SharedContext;
use crate::provenance::PolicyEvaluation;

/// Immutable configuration created during engine construction.
#[derive(Clone)]
struct EngineConfig {
    config: RuntimeConfig,
    default_retry: Option<RetryPolicy>,
    default_timeout: Option<TimeoutPolicy>,
    http_client: HttpClient,
    registry: Registry,
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

        let shared = SharedContext::new(run_id, input.actor_id, self.cfg.registry.clone())
            .with_policies(input.policies.clone());

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

        let status = orchestrator::run_status(&run_output);
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
    async fn get_run(&self, id: Uuid) -> Option<RunSnapshot> {
        self.runs.get_run(id).await
    }

    async fn list_runs(&self, filter: RunFilter) -> Vec<RunSummary> {
        self.runs.list_runs(&filter).await
    }

    async fn cancel_run(&self, id: Uuid) -> Result<(), Error> {
        self.runs.cancel_run(id).await
    }
}
