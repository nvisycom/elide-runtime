//! Per-pass detection pipeline lifecycle.
//!
//! [`DetectionPipeline`] is created per [`Engine::detect`] call.
//! Owns the pass id, registry handle, run context, and
//! [`DetectionState`] accessors.
//!
//! [`Engine::detect`]: crate::pipeline::Engine::detect

use std::sync::Arc;

use jiff::Timestamp;
use nvisy_codec::CodecRegistry;
use nvisy_core::Error;
use nvisy_toolkit::detection::RecognizerRegistry;
use nvisy_toolkit::extraction::ExtractorRegistry;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::input::DetectionInput;
use super::orchestrator::DetectionOrchestrator;
use super::result::DetectionResult;
use super::state::{DetectionRecord, DetectionState};
use super::status::DetectionStatus;
use crate::core::{PolicyStore, RunContext, RunEngines, SharedData};
use crate::document::provenance::AnyAudit;
use crate::phases::ingestion::ImportFile;
use crate::phases::ingestion::encryption::SharedKeyProvider;
use crate::phases::redaction::RedactionRegistries;
use crate::pipeline::RedactionConfig;
use crate::pipeline::config::RuntimeConfig;
use crate::policy::{AnyPolicy, PolicyDigest};
use crate::registry::Registry;

const TARGET: &str = "nvisy_engine::pipeline::detection::pipeline";

/// Pre-built engine resources the detection pipeline borrows for
/// the duration of one pass.
pub(crate) struct DetectionEngineState {
    pub extraction_engine: Arc<ExtractorRegistry>,
    pub recognizer_registry: Arc<RecognizerRegistry>,
    pub redaction_config: Arc<RedactionConfig>,
    pub redaction_registries: Arc<RedactionRegistries>,
}

/// A single detection-pass lifecycle.
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

    pub(crate) fn id(&self) -> Uuid {
        self.detection_id
    }

    /// Build the per-pass [`PolicyStore`] + digests from the
    /// submitted policies and register a pending detection record.
    /// Returns a handle the caller can pass to [`Self::execute`]
    /// carrying the prepared shared state — no further policy
    /// clones happen between this call and orchestrator dispatch.
    pub(crate) async fn register_pending(&self, input: DetectionInput) -> PreparedDetection {
        let actor_id = input.actor_id;
        let policy_digests: Vec<PolicyDigest> =
            input.policies.iter().map(AnyPolicy::digest).collect();
        let policies = Arc::new(PolicyStore::from_any_policies(input.policies));

        self.detections
            .insert(
                self.detection_id,
                DetectionRecord {
                    actor_id,
                    policies: Arc::clone(&policies),
                    policy_digests: policy_digests.clone(),
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

        PreparedDetection {
            actor_id,
            policies,
            policy_digests,
            imports: input.imports,
            plan: input.plan,
        }
    }

    /// Run the detection pass to completion.
    pub(crate) async fn execute(
        &self,
        prepared: PreparedDetection,
    ) -> Result<(Vec<AnyAudit>, u64, DetectionStatus), Error> {
        let actor_id = prepared.actor_id;

        let mut shared_data = SharedData {
            run_id: self.detection_id,
            actor_id,
            policies: prepared.policies,
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
            recognizer_registry: Arc::clone(&self.state.recognizer_registry),
            redaction_config: (*self.state.redaction_config).clone(),
            redaction_registries: (*self.state.redaction_registries).clone(),
        };
        let concurrency = self.base_config.effective_concurrency();
        let ctx = RunContext::new(cancel, Arc::new(shared_data), engines, concurrency);

        self.detections.set_started_at(self.detection_id).await;

        let orchestrator = DetectionOrchestrator::new(ctx);
        let output = match orchestrator.run(&prepared.imports, &prepared.plan).await {
            Ok(out) => out,
            Err(e) => {
                self.detections.fail(self.detection_id, e.to_string()).await;
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

        if matches!(
            status,
            DetectionStatus::Succeeded | DetectionStatus::PartialFailure
        ) {
            let result = DetectionResult {
                id: self.detection_id,
                actor_id,
                policies: prepared.policy_digests,
                imports: prepared.imports.clone(),
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
}

/// Output of [`DetectionPipeline::register_pending`] — the
/// per-pass state that has to flow to [`DetectionPipeline::execute`]
/// without re-touching the original `Vec<AnyPolicy>`.
pub(crate) struct PreparedDetection {
    actor_id: Uuid,
    policies: Arc<PolicyStore>,
    policy_digests: Vec<PolicyDigest>,
    imports: Vec<ImportFile>,
    plan: crate::pipeline::Plan,
}
