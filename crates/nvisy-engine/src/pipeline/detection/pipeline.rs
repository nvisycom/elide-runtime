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
use crate::phases::ingestion::encryption::SharedKeyProvider;
use crate::phases::redaction::RedactionRegistries;
use crate::pipeline::RedactionConfig;
use crate::pipeline::config::RuntimeConfig;
use crate::policy::AnyPolicy;
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
    pub(crate) async fn execute(
        &self,
        input: DetectionInput,
    ) -> Result<(Vec<AnyAudit>, u64, DetectionStatus), Error> {
        let actor_id = input.actor_id;

        let policy_store = build_policy_store(&input.policies);

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
        let output = match orchestrator.run(&input.imports, &input.plan).await {
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
                policies: input.policies.iter().map(AnyPolicy::digest).collect(),
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

}

/// Distribute the modality-erased policies the caller submitted
/// into the typed [`PolicyStore`] buckets the evaluator reads at
/// rule-resolution time. One [`Arc::new`] per policy — the original
/// `AnyPolicy` values are kept alive through the detection record;
/// the store holds independent Arcs purely for the per-run hot
/// path.
pub(crate) fn build_policy_store(policies: &[AnyPolicy]) -> PolicyStore {
    use crate::modality::{Audio, Image, Tabular, Text};

    let mut text = Vec::new();
    let mut tabular = Vec::new();
    let mut image = Vec::new();
    let mut audio = Vec::new();
    for any in policies {
        match any {
            AnyPolicy::Text(p) => text.push(Arc::new(p.clone())),
            AnyPolicy::Tabular(p) => tabular.push(Arc::new(p.clone())),
            AnyPolicy::Image(p) => image.push(Arc::new(p.clone())),
            AnyPolicy::Audio(p) => audio.push(Arc::new(p.clone())),
        }
    }
    let mut store = PolicyStore::new();
    store.set::<Text>(text);
    store.set::<Tabular>(tabular);
    store.set::<Image>(image);
    store.set::<Audio>(audio);
    store
}
