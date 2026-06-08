//! Per-pass redaction pipeline lifecycle.

use std::sync::Arc;

use jiff::Timestamp;
use nvisy_codec::CodecRegistry;
use nvisy_core::Error;
use nvisy_core::modality::Text;
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::applicator::apply_overrides;
use super::input::RedactionInput;
use super::orchestrator::RedactionOrchestrator;
use super::state::{RedactionRecord, RedactionState};
use super::status::RedactionStatus;
use crate::core::{PolicyStore, RunContext, RunEngines, SharedData};
use crate::phases::ingestion::encryption::SharedKeyProvider;
use crate::phases::ingestion::registry::Registry;
use crate::phases::redaction::RedactionRegistries;
use crate::pipeline::RedactionConfig;
use crate::pipeline::config::RuntimeConfig;
use crate::pipeline::detection::DetectionState;
use crate::pipeline::engine::EngineInput;
use crate::policy::Policy;
use crate::provenance::AnyAudit;

const TARGET: &str = "nvisy_document::pipeline::redaction::pipeline";

pub(crate) struct RedactionEngineState {
    pub extraction_engine: Arc<ExtractorRegistry>,
    pub recognizer_registry: Arc<RecognizerRegistry>,
    pub redaction_config: Arc<RedactionConfig>,
    pub redaction_registries: Arc<RedactionRegistries>,
}

pub(crate) struct RedactionPipeline {
    redaction_id: Uuid,
    registry: Registry,
    key_provider: Option<SharedKeyProvider>,
    detections: DetectionState,
    redactions: RedactionState,
    base_config: RuntimeConfig,
    state: RedactionEngineState,
}

impl RedactionPipeline {
    pub(crate) fn new(
        registry: Registry,
        key_provider: Option<SharedKeyProvider>,
        detections: DetectionState,
        redactions: RedactionState,
        base_config: RuntimeConfig,
        state: RedactionEngineState,
    ) -> Self {
        Self {
            redaction_id: Uuid::now_v7(),
            registry,
            key_provider,
            detections,
            redactions,
            base_config,
            state,
        }
    }

    pub(crate) fn id(&self) -> Uuid {
        self.redaction_id
    }

    pub(crate) async fn register_pending(&self, input: &RedactionInput) {
        self.redactions
            .insert(
                self.redaction_id,
                RedactionRecord {
                    detection_id: input.detection_id,
                    actor_id: input.actor_id,
                    status: RedactionStatus::Pending,
                    created_at: Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    cancel: CancellationToken::new(),
                    audits: Vec::new(),
                    redactions_applied: 0,
                    error: None,
                },
            )
            .await;
    }

    /// Drive the redaction to completion.
    pub(crate) async fn execute(&self, input: RedactionInput) -> Result<(), Error> {
        let actor_id = input.actor_id;

        // Load the detection. NotFound / Validation propagate
        // before we mark the redaction Running so the caller's
        // poll sees Pending → Failed atomically.
        let detection = match self
            .detections
            .result(actor_id, input.detection_id)
            .await
        {
            Ok(d) => d,
            Err(e) => {
                self.redactions
                    .fail(self.redaction_id, e.to_string())
                    .await;
                return Err(e);
            }
        };

        // Validate the override batch before doing any work.
        if let Err(e) = super::validate_overrides(&input.overrides) {
            self.redactions
                .fail(self.redaction_id, e.to_string())
                .await;
            return Err(e);
        }

        // Apply overrides to the detection's audits. The audit
        // returned is what the redaction phase will see.
        let mut audits: Vec<AnyAudit> = detection.audits.clone();
        if let Err(e) = apply_overrides(&mut audits, input.overrides.clone()) {
            self.redactions
                .fail(self.redaction_id, e.to_string())
                .await;
            return Err(e);
        }

        // Build per-pass context.
        let policy_ids = detection.policies.clone();
        let _policy_guard = self.acquire_policies(actor_id, &policy_ids).await;
        let cached_policies = self.registry.policy_cache().resolve(&policy_ids).await;
        let text_policies: Vec<Arc<Policy<Text>>> = cached_policies;

        let mut policy_store = PolicyStore::new();
        policy_store.set::<Text>(text_policies);

        let mut shared_data = SharedData {
            run_id: self.redaction_id,
            actor_id,
            policies: policy_store,
            registry: self.registry.clone(),
            codec_registry: CodecRegistry::with_builtin(),
            key_provider: SharedKeyProvider::default(),
        };
        if let Some(ref kp) = self.key_provider {
            shared_data.key_provider = kp.clone();
        }

        let cancel = CancellationToken::new();
        let engines = RunEngines {
            extraction_engine: (*self.state.extraction_engine).clone(),
            recognizer_registry: (*self.state.recognizer_registry).clone(),
            redaction_config: (*self.state.redaction_config).clone(),
            redaction_registries: (*self.state.redaction_registries).clone(),
        };
        let concurrency = self.base_config.effective_concurrency();
        // dry_run=false: redaction wants the apply path.
        let ctx = RunContext::new(
            cancel,
            Arc::new(shared_data),
            engines,
            concurrency,
            false,
        );

        // Build the legacy EngineInput shape. Imports come from
        // the detection; exports from the redaction request.
        let engine_input = EngineInput {
            actor_id,
            policies: detection.policies.clone(),
            dry_run: false,
            imports: detection.imports.clone(),
            plan: crate::core::Plan::default(),
            exports: input.exports.clone(),
        };

        self.redactions.set_started_at(self.redaction_id).await;

        let orchestrator = RedactionOrchestrator::new(ctx);
        let output = match orchestrator.run(&engine_input, audits).await {
            Ok(out) => out,
            Err(e) => {
                self.redactions
                    .fail(self.redaction_id, e.to_string())
                    .await;
                return Err(e);
            }
        };

        let (final_audits, redactions_applied, any_ok, any_failed) = output.into_audits();
        let status = if !any_failed {
            RedactionStatus::Succeeded
        } else if any_ok {
            RedactionStatus::PartialFailure
        } else {
            RedactionStatus::Failed
        };

        // Persist the redaction result for terminal-with-data
        // statuses. Same durability story as detection: write
        // to disk first, then flip in-memory state.
        if matches!(
            status,
            RedactionStatus::Succeeded | RedactionStatus::PartialFailure
        ) {
            let result = crate::pipeline::redaction::RedactionResult {
                id: self.redaction_id,
                detection_id: input.detection_id,
                actor_id,
                audits: final_audits.clone(),
                redactions_applied,
            };
            if let Err(e) = self
                .registry
                .store_redaction(actor_id, self.redaction_id, &result)
                .await
            {
                tracing::error!(
                    target: TARGET,
                    redaction_id = %self.redaction_id,
                    error = %e,
                    "failed to persist redaction result; in-memory state stays valid",
                );
            }
        }

        self.redactions
            .finalize(
                self.redaction_id,
                status,
                final_audits,
                redactions_applied,
            )
            .await;

        tracing::info!(
            target: TARGET,
            redaction_id = %self.redaction_id,
            detection_id = %input.detection_id,
            ?status,
            redactions_applied,
            "redaction pass finalized",
        );

        Ok(())
    }

    async fn acquire_policies(
        &self,
        actor_id: Uuid,
        policy_ids: &[Uuid],
    ) -> crate::phases::ingestion::registry::ResourceGuard<Policy<Text>> {
        self.registry
            .policy_cache()
            .acquire(policy_ids, |id| async move {
                match self.registry.read_policy(actor_id, id).await {
                    Ok(policy) => Some(policy),
                    Err(e) => {
                        tracing::warn!(%id, error = %e, "failed to load policy");
                        None
                    }
                }
            })
            .await
    }
}
