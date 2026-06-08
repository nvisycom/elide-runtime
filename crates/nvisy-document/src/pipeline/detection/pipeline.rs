//! Per-pass detection pipeline lifecycle.
//!
//! [`DetectionPipeline`] is created per `Engine::detect` call.
//! Owns the pass id, registry handle, run context, and
//! `DetectionState` accessors. Mirrors the legacy
//! `pipeline/run.rs::Pipeline` shape.

use std::sync::Arc;

use jiff::Timestamp;
use nvisy_codec::CodecRegistry;
use nvisy_core::Error;
use nvisy_core::modality::Text;
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::input::DetectionInput;
use super::orchestrator::DetectionOrchestrator;
use super::state::{DetectionRecord, DetectionState};
use super::status::DetectionStatus;
use crate::core::{PolicyStore, RunContext, RunEngines, SharedData};
use crate::phases::ingestion::encryption::SharedKeyProvider;
use crate::phases::ingestion::registry::Registry;
use crate::phases::redaction::RedactionRegistries;
use crate::pipeline::RedactionConfig;
use crate::pipeline::config::RuntimeConfig;
use crate::pipeline::engine::EngineInput;
use crate::policy::Policy;

const TARGET: &str = "nvisy_document::pipeline::detection::pipeline";

/// Pre-built engine resources the detection pipeline borrows for
/// the duration of one pass. Bundled so [`DetectionPipeline::new`]
/// stays narrow.
pub(crate) struct DetectionEngineState {
    pub extraction_engine: Arc<ExtractorRegistry>,
    pub recognizer_registry: Arc<RecognizerRegistry>,
    pub redaction_config: Arc<RedactionConfig>,
    pub redaction_registries: Arc<RedactionRegistries>,
}

/// A single detection-pass lifecycle.
///
/// Created per pass, not reusable. Owns the pass id and the
/// state-mutator handles needed to drive the pass through
/// registration → execution → finalization.
pub(crate) struct DetectionPipeline {
    detection_id: Uuid,
    registry: Registry,
    key_provider: Option<SharedKeyProvider>,
    detections: DetectionState,
    base_config: RuntimeConfig,
    state: DetectionEngineState,
}

impl DetectionPipeline {
    pub(crate) fn new(
        registry: Registry,
        key_provider: Option<SharedKeyProvider>,
        detections: DetectionState,
        base_config: RuntimeConfig,
        state: DetectionEngineState,
    ) -> Self {
        Self {
            detection_id: Uuid::now_v7(),
            registry,
            key_provider,
            detections,
            base_config,
            state,
        }
    }

    /// The unique id assigned to this detection pass.
    pub(crate) fn id(&self) -> Uuid {
        self.detection_id
    }

    /// Insert the initial `Pending` record so callers can
    /// observe the pass before it starts executing.
    pub(crate) async fn register_pending(&self, input: &DetectionInput) {
        self.detections
            .insert(
                self.detection_id,
                DetectionRecord {
                    actor_id: input.actor_id,
                    policies: input.policies.clone(),
                    imports: input.imports.clone(),
                    status: DetectionStatus::Pending,
                    created_at: Timestamp::now(),
                    started_at: None,
                    completed_at: None,
                    cancel: CancellationToken::new(),
                    audits: Vec::new(),
                    entities_detected: 0,
                    error: None,
                },
            )
            .await;
    }

    /// Run the detection pass to completion.
    ///
    /// Loads policies, builds the per-pass context, fans documents
    /// out to the [`DetectionOrchestrator`], collects audits, and
    /// updates the `DetectionState` to the appropriate terminal
    /// status. Returns the persisted audits on success.
    pub(crate) async fn execute(
        &self,
        input: DetectionInput,
    ) -> Result<(Vec<crate::provenance::AnyAudit>, u64, DetectionStatus), Error> {
        let actor_id = input.actor_id;
        let policy_ids = input.policies.clone();

        // Acquire policies into the registry cache; guard keeps
        // them alive for the duration of the pass.
        let _policy_guard = self.acquire_policies(actor_id, &policy_ids).await;
        let cached_policies = self.registry.policy_cache().resolve(&policy_ids).await;
        let text_policies: Vec<Arc<Policy<Text>>> = cached_policies;

        let mut policy_store = PolicyStore::new();
        policy_store.set::<Text>(text_policies);

        let mut shared_data = SharedData {
            run_id: self.detection_id,
            actor_id,
            policies: policy_store,
            registry: self.registry.clone(),
            codec_registry: CodecRegistry::with_builtin(),
            key_provider: SharedKeyProvider::default(),
        };
        if let Some(ref kp) = self.key_provider {
            shared_data.key_provider = kp.clone();
        }

        let recognizer_registry = (*self.state.recognizer_registry).clone();
        let cancel = CancellationToken::new();
        let engines = RunEngines {
            extraction_engine: (*self.state.extraction_engine).clone(),
            recognizer_registry,
            redaction_config: (*self.state.redaction_config).clone(),
            redaction_registries: (*self.state.redaction_registries).clone(),
        };
        let concurrency = self.base_config.effective_concurrency();
        // Detection never touches the redaction phase, so the
        // `dry_run` flag's semantics ("skip redaction / validation
        // / export") match what we want. This coupling goes away
        // when `EngineInput` is deleted (task #462).
        let ctx = RunContext::new(
            cancel,
            Arc::new(shared_data),
            engines,
            concurrency,
            true,
        );

        // Build the legacy-shaped EngineInput from the typed
        // DetectionInput. Empty exports, dry_run=true. The
        // orchestrator only reads `imports` and `plan`.
        let engine_input = EngineInput {
            actor_id,
            policies: input.policies.clone(),
            dry_run: true,
            imports: input.imports.clone(),
            plan: input.plan,
            exports: Vec::new(),
        };

        self.detections.set_started_at(self.detection_id).await;

        let orchestrator = DetectionOrchestrator::new(ctx);
        let output = match orchestrator.run(&engine_input).await {
            Ok(out) => out,
            Err(e) => {
                self.detections
                    .fail(self.detection_id, e.to_string())
                    .await;
                return Err(e);
            }
        };

        let (audits, entities_detected, any_ok, any_failed) = output.into_audits();
        let status = if !any_failed {
            DetectionStatus::Succeeded
        } else if any_ok {
            DetectionStatus::PartialFailure
        } else {
            DetectionStatus::Failed
        };

        // Persist before flipping in-memory state to terminal —
        // a crash mid-write leaves the in-memory record at
        // `Running` (which a restart will rebuild from disk
        // showing the persisted state or absence thereof).
        // Failure to persist is logged but doesn't fail the
        // pass; the in-memory state still reflects what was
        // computed. This is acceptable because the in-memory
        // record is volatile by design — persistence is the
        // durability boundary.
        if matches!(
            status,
            DetectionStatus::Succeeded | DetectionStatus::PartialFailure
        ) {
            let result = crate::pipeline::detection::DetectionResult {
                id: self.detection_id,
                actor_id,
                policies: input.policies.clone(),
                imports: input.imports.clone(),
                audits: audits.clone(),
                entities_detected,
            };
            if let Err(e) = self
                .registry
                .store_detection(actor_id, self.detection_id, &result)
                .await
            {
                tracing::error!(
                    target: TARGET,
                    detection_id = %self.detection_id,
                    error = %e,
                    "failed to persist detection result; in-memory state stays valid",
                );
            }
        }

        self.detections
            .finalize(self.detection_id, status, audits.clone(), entities_detected)
            .await;

        tracing::info!(
            target: TARGET,
            detection_id = %self.detection_id,
            ?status,
            entities_detected,
            "detection pass finalized",
        );

        Ok((audits, entities_detected, status))
    }

    /// Acquire policies into the registry's read cache.
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
