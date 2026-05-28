//! Single pipeline run lifecycle.
//!
//! [`Pipeline`] encapsulates the full lifecycle of one pipeline run:
//! resource acquisition, execution, retention enforcement, and
//! finalization. It is created per-run by [`Engine::run`] or
//! [`Engine::submit`] and is not reusable.
//!
//! [`Engine::run`]: super::Engine::run
//! [`Engine::submit`]: super::Engine::submit

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_ontology::modality::Text;
use nvisy_ontology::policy::{Retention, RetentionPolicy, RetentionScope};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::default::{EngineInput, EngineOutput};
use super::orchestrator::{Orchestrator, RunContext};
use super::runs::RunStatus;
use super::runs::state::{RunRecord, RunState};
use crate::detection::Recognizers;
use crate::envelope::SharedData;
use crate::extraction::Extractors;
use crate::ingestion::encryption::SharedKeyProvider;
use crate::ingestion::registry::Registry;
use crate::redaction::RedactionDefaults;

const TARGET: &str = "nvisy_engine::pipeline::run";

/// A single pipeline run lifecycle.
///
/// Created per-run, not reusable. Owns the run ID, cancellation
/// token, and all intermediate state needed to drive the run from
/// preparation through finalization.
pub(super) struct Pipeline {
    run_id: Uuid,
    registry: Registry,
    key_provider: Option<SharedKeyProvider>,
    runs: RunState,
    base_config: RuntimeConfig,
    extractors: Arc<Extractors>,
    recognizers: Arc<Recognizers>,
    redaction_defaults: Arc<RedactionDefaults>,
}

impl Pipeline {
    /// Create a new pipeline run bound to the given engine state.
    pub fn new(
        registry: Registry,
        key_provider: Option<SharedKeyProvider>,
        runs: RunState,
        base_config: RuntimeConfig,
        extractors: Arc<Extractors>,
        recognizers: Arc<Recognizers>,
        redaction_defaults: Arc<RedactionDefaults>,
    ) -> Self {
        Self {
            run_id: Uuid::now_v7(),
            registry,
            key_provider,
            runs,
            base_config,
            extractors,
            recognizers,
            redaction_defaults,
        }
    }

    /// The unique identifier for this run.
    pub fn id(&self) -> Uuid {
        self.run_id
    }

    /// Register + execute the run end-to-end.
    ///
    /// This is the synchronous (blocking) path used by [`Engine::run`].
    ///
    /// [`Engine::run`]: super::Engine::run
    pub async fn run(self, input: EngineInput) -> Result<EngineOutput, Error> {
        self.register_pending(&input).await;
        self.execute(input).await
    }

    /// Insert the initial run record as `Pending`. Called separately
    /// from [`execute`] so [`Engine::submit`] can return the run_id
    /// before spawning the background task.
    ///
    /// [`execute`]: Self::execute
    /// [`Engine::submit`]: super::Engine::submit
    pub async fn register_pending(&self, input: &EngineInput) {
        self.runs
            .insert(
                self.run_id,
                RunRecord {
                    actor_id: input.actor_id,
                    status: RunStatus::Pending,
                    created_at: jiff::Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    nodes: HashMap::new(),
                    cancel: CancellationToken::new(),
                    entities_detected: 0,
                    redactions_applied: 0,
                    error: None,
                },
            )
            .await;
    }

    /// Run the pipeline to completion.
    pub async fn execute(&self, input: EngineInput) -> Result<EngineOutput, Error> {
        let effective_config = match &input.config {
            Some(overrides) => self.base_config.merge(overrides),
            None => self.base_config.clone(),
        };

        let actor_id = input.actor_id;

        // Sort policy refs by precedence (lower first); the stable sort
        // preserves insertion order for equal-precedence refs.
        let mut policy_refs = input.policies.clone();
        policy_refs.sort_by_key(|r| r.precedence);
        let policy_ids: Vec<Uuid> = policy_refs.iter().map(|r| r.id).collect();

        // Acquire contexts and policies into the registry caches.
        let (_context_guard, _policy_guard) = self
            .acquire_resources(actor_id, &input.context_ids, &policy_ids)
            .await;

        let cached_policies = self.registry.policy_cache().resolve(&policy_ids).await;
        // Registry holds Policy<Text> only today (#199 will widen
        // storage to multi-modality via PolicyStore on the cache).
        let mut text_policies: Vec<nvisy_ontology::policy::Policy<Text>> = Vec::new();
        for policy in cached_policies {
            text_policies.push(Arc::unwrap_or_clone(policy));
        }

        let retention_rules: Vec<RetentionPolicy> = text_policies
            .iter()
            .flat_map(|p| p.retention.iter().copied())
            .collect();

        let concurrency = effective_config.effective_concurrency();

        let mut policy_store = crate::envelope::PolicyStore::new();
        policy_store.set::<Text>(text_policies);

        let mut shared_data = SharedData {
            run_id: self.run_id,
            actor_id,
            policies: policy_store,
            registry: self.registry.clone(),
            key_provider: SharedKeyProvider::default(),
        };
        if let Some(ref kp) = self.key_provider {
            shared_data.key_provider = kp.clone();
        }

        // Build the detection engine once per run by picking
        // pre-built recognizers from the registry. `None` when the
        // workflow opted no recognizers in (`kinds` empty) — we
        // skip the detection phase rather than fail.
        let detection_engine = if input.detection.kinds.is_empty() {
            None
        } else {
            Some(Arc::new(
                input
                    .detection
                    .into_engine(&self.recognizers)
                    .map_err(|e| {
                        Error::validation(format!("detection engine assembly: {e}"), "detection")
                    })?,
            ))
        };

        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        // Per-request redactor overrides win; otherwise reuse the
        // pre-built defaults. We always rebuild a fresh Arc when the
        // effective config carries a section, since identity-equality
        // doesn't tell us whether the section came from override or
        // base. The defaults struct is small (two scalars + an
        // Option), so the allocation is trivial.
        let redaction_defaults = match &effective_config.redaction {
            Some(d) => Arc::new(d.clone()),
            None => Arc::clone(&self.redaction_defaults),
        };
        let ctx = RunContext {
            cancel,
            shared: Arc::new(shared_data),
            extractors: Arc::clone(&self.extractors),
            detection_engine,
            redaction_defaults,
            concurrency,
            dry_run: input.dry_run,
        };

        self.runs.set_started_at(self.run_id).await;

        let limits = self.base_config.effective_limits();
        let orchestrator = Orchestrator::new(ctx);
        let run_output = if let Some(duration) = limits.run_timeout {
            match tokio::time::timeout(duration, orchestrator.run(&input)).await {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    self.runs.fail(self.run_id, e.to_string()).await;
                    return Err(e);
                }
                Err(_) => {
                    cancel_clone.cancel();
                    self.runs
                        .fail(self.run_id, "pipeline run exceeded time limit")
                        .await;
                    return Err(Error::timeout("pipeline run exceeded time limit"));
                }
            }
        } else {
            match orchestrator.run(&input).await {
                Ok(output) => output,
                Err(e) => {
                    self.runs.fail(self.run_id, e.to_string()).await;
                    return Err(e);
                }
            }
        };

        // Collect audits and counters from document results.
        let mut audits = Vec::new();
        let mut entities_detected = 0u64;
        let mut redactions_applied = 0u64;
        let mut any_failed = false;
        let mut any_ok = false;

        for result in &run_output.results {
            if result.error.is_some() {
                any_failed = true;
                continue;
            }
            any_ok = true;
            if let Some(ref envelope) = result.envelope {
                let audit = envelope.audit_cloned();
                entities_detected += audit.entities_count() as u64;
                redactions_applied += audit.applied_redactions_count() as u64;
                audits.push(audit);
            }
        }

        let mut output = EngineOutput {
            run_id: self.run_id,
            audits,
        };

        // Enforce retention policies.
        self.apply_retention(actor_id, &retention_rules, &input, &mut output)
            .await;

        let status = if !any_failed {
            RunStatus::Succeeded
        } else if any_ok {
            RunStatus::PartialFailure
        } else {
            RunStatus::Failed
        };

        self.runs
            .finalize(self.run_id, status, entities_detected, redactions_applied)
            .await;

        tracing::info!(
            target: TARGET,
            run_id = %self.run_id,
            ?status,
            entities_detected,
            redactions_applied,
            "pipeline run finalized",
        );

        Ok(output)
    }

    /// Acquire contexts and policies into the registry caches.
    async fn acquire_resources(
        &self,
        actor_id: Uuid,
        context_ids: &[Uuid],
        policy_ids: &[Uuid],
    ) -> (
        crate::ingestion::registry::ResourceGuard<nvisy_ontology::context::Context>,
        crate::ingestion::registry::ResourceGuard<nvisy_ontology::policy::Policy<Text>>,
    ) {
        let registry = &self.registry;

        let context_guard = registry
            .context_cache()
            .acquire(context_ids, |id| async move {
                match registry.read_context(actor_id, id).await {
                    Ok(ctx) => Some(ctx),
                    Err(e) => {
                        tracing::warn!(%id, error = %e, "failed to load context");
                        None
                    }
                }
            })
            .await;

        let policy_guard = registry
            .policy_cache()
            .acquire(policy_ids, |id| async move {
                match registry.read_policy(actor_id, id).await {
                    Ok(policy) => Some(policy),
                    Err(e) => {
                        tracing::warn!(%id, error = %e, "failed to load policy");
                        None
                    }
                }
            })
            .await;

        (context_guard, policy_guard)
    }

    /// Enforce retention policies after a pipeline run.
    async fn apply_retention(
        &self,
        actor_id: Uuid,
        retention_rules: &[RetentionPolicy],
        input: &EngineInput,
        output: &mut EngineOutput,
    ) {
        if retention_rules.is_empty() {
            return;
        }

        for rp in retention_rules {
            if !matches!(rp.retention, Retention::ZeroRetention) {
                continue;
            }
            match rp.scope {
                RetentionScope::OriginalContent => {
                    let content_ids: Vec<Uuid> = input
                        .imports
                        .iter()
                        .flat_map(|cfg| cfg.content_ids.clone())
                        .collect();
                    for id in content_ids {
                        if let Err(e) = self.registry.unregister_content(actor_id, id).await {
                            tracing::warn!(
                                %id,
                                error = %e,
                                "failed to delete content for zero-retention",
                            );
                        }
                    }
                }
                RetentionScope::RedactedOutput => {
                    let content_ids: Vec<Uuid> = input
                        .exports
                        .iter()
                        .flat_map(|cfg| cfg.content_ids.clone())
                        .collect();
                    for id in content_ids {
                        if let Err(e) = self.registry.unregister_content(actor_id, id).await {
                            tracing::warn!(
                                %id,
                                error = %e,
                                "failed to delete exported content for zero-retention",
                            );
                        }
                    }
                }
                RetentionScope::AuditLogs => {
                    output.audits.clear();
                    if let Err(e) = self
                        .registry
                        .unregister_audits(actor_id, output.run_id)
                        .await
                    {
                        tracing::warn!(
                            run_id = %output.run_id,
                            error = %e,
                            "failed to delete persisted audits for zero-retention",
                        );
                    }
                }
                _ => {}
            }
        }
    }
}
