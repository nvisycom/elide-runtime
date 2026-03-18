//! Node-level execution dispatchers.
//!
//! [`NodeExecutor`] dispatches each graph node to the appropriate handler
//! based on its [`GraphNodeKind`]. Pre-compiled timeout and retry policies
//! from the [`ResolvedNode`] are applied directly, with
//! [`TimeoutBehavior`] controlling whether a timeout is treated as an error
//! or silently yields zero items. Retry wraps each individual item
//! processed by the node (not the node as a whole), so channel-based
//! fan-out is never re-consumed.

use std::sync::Arc;

use futures::StreamExt;
use nvisy_codec::handler::{Handler, ImageHandler, TextData, TextHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use nvisy_registry::Registry;
use nvisy_rig::agent::OcrAgent;
use nvisy_rig::audio::stt::{SttConfig, SttService};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::config::RuntimeConfig;
use super::plan::ResolvedNode;
use super::policy::CompiledRetryPolicy;
use crate::graph::{self, GraphNodeKind};
use crate::graph::policy::TimeoutBehavior;
use crate::operation::envelope::DetectedEntities;
use crate::operation::inference::{Ner, NerMethodParams, Ocr, OcrVerification, OcrVerificationInput};
use crate::operation::lifecycle::Import;
use crate::operation::processing::{
    Deduplication, Ensemble, EvaluatePolicy, EvaluatePolicyParams, FusionStrategy,
    PatternDetectionParams, PatternMatch,
};
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SequentialContext, SharedContext};

const TARGET: &str = "nvisy_engine::pipeline::executor";

/// Outcome of executing a single node in the pipeline.
#[derive(Debug)]
pub(super) struct NodeOutput {
    /// ID of the node that produced this result.
    pub node_id: Uuid,
    /// Number of data items processed by this node.
    pub items_processed: u64,
    /// Error message if the node failed, or `None` on success.
    pub error: Option<String>,
    /// Envelopes collected by terminal (export) nodes.
    pub envelopes: Vec<Arc<DocumentEnvelope>>,
}

/// Aggregate outcome of executing an entire pipeline graph.
#[derive(Debug)]
pub(super) struct RunOutput {
    /// Per-node results in completion order.
    pub node_results: Vec<NodeOutput>,
}

/// Registry + content identifiers needed only by Import nodes.
#[derive(Clone)]
pub(super) struct ImportSource {
    pub registry: Registry,
    pub content_ids: Arc<[Uuid]>,
}

/// Executes a single resolved node within a pipeline run.
pub(super) struct NodeExecutor {
    shared: SharedContext,
    cancel: CancellationToken,
    config: RuntimeConfig,
    http_client: HttpClient,
    import_source: Option<ImportSource>,
}

impl NodeExecutor {
    pub fn new(
        shared: SharedContext,
        cancel: CancellationToken,
        config: RuntimeConfig,
        http_client: HttpClient,
    ) -> Self {
        Self {
            shared,
            cancel,
            config,
            http_client,
            import_source: None,
        }
    }

    pub fn with_import_source(mut self, source: ImportSource) -> Self {
        self.import_source = Some(source);
        self
    }

    /// Execute a resolved node, applying timeout and cancellation policies.
    pub async fn execute(
        &self,
        resolved: &ResolvedNode,
        senders: Vec<mpsc::Sender<Arc<DocumentEnvelope>>>,
        mut receivers: Vec<mpsc::Receiver<Arc<DocumentEnvelope>>>,
    ) -> Result<NodeOutput, Error> {
        if self.cancel.is_cancelled() {
            return Err(Error::cancellation("run cancelled"));
        }

        let node_id = resolved.node.id;
        let cancel = self.cancel.clone();

        let run = async {
            tokio::select! {
                _ = cancel.cancelled() => {
                    Err(Error::cancellation("run cancelled"))
                }
                result = self.dispatch(
                    node_id,
                    &resolved.node.kind,
                    resolved.compiled_retry.as_ref(),
                    &senders,
                    &mut receivers,
                ) => {
                    result
                }
            }
        };

        match &resolved.compiled_timeout {
            Some(compiled) => {
                let result: Result<NodeOutput, Error> = compiled.with_timeout(run).await;
                match (&result, &compiled.on_timeout) {
                    (Err(e), TimeoutBehavior::Skip) if e.kind == ErrorKind::Timeout => {
                        Ok(NodeOutput {
                            node_id,
                            items_processed: 0,
                            error: None,
                            envelopes: Vec::new(),
                        })
                    }
                    _ => result,
                }
            }
            None => run.await,
        }
    }

    /// Dispatch based on node kind.
    async fn dispatch(
        &self,
        node_id: Uuid,
        action: &GraphNodeKind,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        match action {
            GraphNodeKind::Import(cfg) => {
                self.execute_import(node_id, cfg, retry, senders).await
            }
            GraphNodeKind::Export(cfg) => {
                self.execute_export(node_id, cfg, receivers).await
            }
            GraphNodeKind::VisualExtraction(cfg) => {
                self.execute_visual_extraction(node_id, cfg, retry, senders, receivers)
                    .await
            }
            GraphNodeKind::AudialExtraction(cfg) => {
                self.execute_audial_extraction(node_id, cfg, retry, senders, receivers)
                    .await
            }
            GraphNodeKind::NamedEntityRecognition(cfg) => {
                self.execute_ner(node_id, cfg, retry, senders, receivers)
                    .await
            }
            GraphNodeKind::PatternRecognition(_) => {
                self.execute_pattern_recognition(node_id, retry, senders, receivers)
                    .await
            }
            GraphNodeKind::Fusion(cfg) => {
                self.execute_fusion(node_id, cfg, retry, senders, receivers)
                    .await
            }
            GraphNodeKind::Redaction(cfg) => {
                self.execute_redaction(node_id, cfg, retry, senders, receivers)
                    .await
            }
            GraphNodeKind::LoadContext(_)
            | GraphNodeKind::SaveContext(_) => {
                self.execute_context_passthrough(node_id, action, senders, receivers)
                    .await
            }
            GraphNodeKind::GenerateContext(cfg) => {
                self.execute_generate_context(node_id, cfg, senders, receivers)
                    .await
            }
        }
    }

    async fn execute_import(
        &self,
        node_id: Uuid,
        cfg: &graph::Import,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let source = self.import_source.as_ref().ok_or_else(|| {
            Error::new(ErrorKind::Internal, "import node has no content source")
        })?;

        if cfg.decompression {
            tracing::warn!(target: TARGET, "import decompression requested but Decompression operation is not yet implemented, skipping");
        }
        if cfg.decryption {
            tracing::warn!(target: TARGET, "import decryption requested but KeyProvider is not yet configurable, skipping");
        }
        if cfg.conversion {
            tracing::warn!(target: TARGET, "import conversion requested but Conversion operation is not yet implemented, skipping");
        }

        let actor_id = self.shared.actor_id;
        let mut count = 0u64;

        for &content_id in source.content_ids.iter() {
            let envelope = self
                .import_one(&source.registry, actor_id, content_id, retry)
                .await?;
            let envelope = Arc::new(envelope);

            for tx in senders {
                let _ = tx.send(Arc::clone(&envelope)).await;
            }
            drop(envelope);
            count += 1;
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn import_one(
        &self,
        registry: &Registry,
        actor_id: Uuid,
        content_id: Uuid,
        retry: Option<&CompiledRetryPolicy>,
    ) -> Result<DocumentEnvelope, Error> {
        let do_import = || async {
            let handle = registry.read_content(actor_id, content_id).await?;
            let content_data = handle.content_data().await?;
            let input = ParallelContext::new(content_data, self.shared.clone());
            let output = Import.call(input).await?;
            Ok(output.into_inner())
        };

        match retry {
            Some(policy) => policy.with_retry(do_import).await,
            None => do_import().await,
        }
    }

    async fn execute_export(
        &self,
        node_id: Uuid,
        cfg: &graph::Export,
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        if cfg.compression {
            tracing::warn!(target: TARGET, "export compression requested but Compression operation is not yet implemented, skipping");
        }
        if cfg.encryption {
            tracing::warn!(target: TARGET, "export encryption requested but KeyProvider is not yet configurable, skipping");
        }
        if cfg.conversion {
            tracing::warn!(target: TARGET, "export conversion requested but Conversion operation is not yet implemented, skipping");
        }

        let mut count = 0u64;
        let mut envelopes = Vec::new();

        for rx in receivers.iter_mut() {
            while let Some(envelope) = rx.recv().await {
                count += 1;
                envelopes.push(envelope);
            }
        }

        tracing::debug!(target: TARGET, count, "export node collected envelopes");

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes,
        })
    }

    async fn execute_visual_extraction(
        &self,
        node_id: Uuid,
        cfg: &graph::VisualExtraction,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let ocr_section = self.config.ocr.as_ref();
        let ocr_provider = ocr_section
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "visual_extraction requires an OCR provider in config",
                )
            })?;
        let ocr_params = ocr_section
            .and_then(|s| s.policy.clone())
            .unwrap_or_default();
        let ocr_engine = ocr_provider.into_engine_with_client(self.http_client.clone());
        let ocr = Ocr::new(ocr_engine, ocr_params);

        // Build OcrVerification agent if verification is requested and LLM is available.
        let ocr_verifier = if cfg.verification {
            match self.build_ocr_agent() {
                Ok(agent) => Some(OcrVerification::new(agent)),
                Err(e) => {
                    tracing::warn!(target: TARGET, error = %e, "OCR verification requested but agent could not be built, skipping verification");
                    None
                }
            }
        } else {
            None
        };

        // Computer vision entity detection requires a CvProvider impl which is not
        // yet exposed through RuntimeConfig. Log and skip when requested.
        if cfg.entity_detection {
            tracing::warn!(target: TARGET, "entity_detection requested but CvProvider is not yet configurable, skipping computer vision");
        }

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                let mut envelope = unwrap_envelope(item).await?;

                if let Document::Image(ref handler) = envelope.document {
                    let image_spans: Vec<_> = handler.image_spans().await.collect().await;
                    let ocr_spans: Vec<Span<(), _>> = image_spans
                        .into_iter()
                        .map(|s| Span::new((), s.data).with_source(s.source))
                        .collect();

                    let ocr_ref = &ocr;
                    let do_ocr = || {
                        let spans = ocr_spans.clone();
                        let shared = self.shared.clone();
                        async move {
                            let input = ParallelContext::new(spans, shared);
                            ocr_ref.call(input).await
                        }
                    };
                    let ocr_output = call_with_retry(retry, do_ocr).await?;

                    let mut entities = nvisy_ontology::entity::Entities::new();
                    for img_out in ocr_output.into_inner() {
                        for word in img_out.words() {
                            if let Some(entity) = word_to_entity(word) {
                                entities.push(entity);
                            }
                        }
                    }
                    if !entities.is_empty() {
                        envelope.apply(DetectedEntities(entities));
                    }

                    // Run OCR verification if enabled: verify detected entities against images.
                    if let Some(ref verifier) = ocr_verifier
                        && !envelope.entities.is_empty()
                    {
                        let image_spans_for_verify: Vec<_> = {
                            let handler = match &envelope.document {
                                Document::Image(h) => h,
                                _ => unreachable!(),
                            };
                            handler
                                .image_spans()
                                .await
                                .collect::<Vec<_>>()
                                .await
                                .into_iter()
                                .map(|s| Span::new((), s.data).with_source(s.source))
                                .collect()
                        };

                        let verify_input = OcrVerificationInput {
                            image_spans: image_spans_for_verify,
                            entities: envelope.entities.clone(),
                        };

                        let verifier_ref = verifier;
                        let do_verify = || {
                            let input_clone = OcrVerificationInput {
                                image_spans: verify_input.image_spans.clone(),
                                entities: verify_input.entities.clone(),
                            };
                            let shared = self.shared.clone();
                            async move {
                                let input = ParallelContext::new(input_clone, shared);
                                verifier_ref.call(input).await
                            }
                        };
                        match call_with_retry(retry, do_verify).await {
                            Ok(output) => {
                                envelope.apply(output.into_inner());
                            }
                            Err(e) => {
                                tracing::warn!(
                                    target: TARGET,
                                    error = %e,
                                    "OCR verification failed, keeping unverified entities"
                                );
                            }
                        }
                    }
                }

                count += 1;
                let envelope = Arc::new(envelope);
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_audial_extraction(
        &self,
        node_id: Uuid,
        cfg: &graph::AudialExtraction,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let stt_section = self.config.stt.as_ref();
        let stt_provider = stt_section
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "audial_extraction requires an STT provider in config",
                )
            })?;

        let stt_config = SttConfig::default();
        let stt = SttService::with_http_client(&stt_provider, stt_config, self.http_client.clone())
            .map_err(|e: nvisy_rig::error::Error| Error::runtime(e.to_string(), "stt-service", false))?;

        if cfg.diarization {
            tracing::warn!(target: TARGET, "diarization requested but not yet supported by SttService, skipping");
        }

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                let envelope = unwrap_envelope(item).await?;

                if let Document::Audio(ref handler) = envelope.document {
                    let audio_data = Handler::encode(handler)?;
                    let filename: String = audio_data.filename
                        .as_deref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "audio.wav".to_string());

                    let stt_ref = &stt;
                    let do_transcribe = || {
                        let bytes = audio_data.as_bytes().to_vec();
                        let fname = filename.clone();
                        async move {
                            let output = stt_ref.transcribe(&bytes, &fname).await
                                .map_err(|e: nvisy_rig::error::Error| Error::runtime(e.to_string(), "stt-transcribe", e.is_retryable()))?;
                            Ok::<_, Error>(output)
                        }
                    };
                    let stt_output = call_with_retry(retry, do_transcribe).await?;

                    tracing::debug!(
                        target: TARGET,
                        text_len = stt_output.text.len(),
                        "transcription complete for audio document"
                    );
                    // Transcribed text is available but not yet injected back
                    // into the envelope because there's no standard enrichment
                    // path for extracted text on audio documents. Downstream NER
                    // nodes will need the transcribed text — this will be wired
                    // when the envelope gains a `transcriptions` field.
                }

                count += 1;
                let envelope = Arc::new(envelope);
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_ner(
        &self,
        node_id: Uuid,
        cfg: &graph::NamedEntityRecognition,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let llm_section = self.config.llm.as_ref();
        let provider = llm_section
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "ner requires an LLM provider in config",
                )
            })?;
        let agent_config = llm_section
            .and_then(|s| s.policy.clone())
            .unwrap_or_default();

        let ner = Ner::connect(NerMethodParams {
            entity_kinds: cfg.entity_kinds.clone(),
            confidence_threshold: cfg.confidence_threshold.unwrap_or(0.5),
            provider: Some(provider),
            agent_config: Some(agent_config),
            http_client: Some(self.http_client.clone()),
        })
        .await?;

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                let mut envelope = unwrap_envelope(item).await?;

                let text_spans = collect_ner_spans(&envelope.document).await;
                if !text_spans.is_empty() {
                    let ner_ref = &ner;
                    let do_ner = || {
                        let spans = text_spans.clone();
                        let shared = self.shared.clone();
                        async move {
                            let input = SequentialContext::new(spans, shared);
                            ner_ref.call(input).await
                        }
                    };
                    let ner_output = call_with_retry(retry, do_ner).await?;
                    envelope.apply(ner_output.into_inner());
                }

                ner.reset().await;
                count += 1;
                let envelope = Arc::new(envelope);
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_pattern_recognition(
        &self,
        node_id: Uuid,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let pattern_match = PatternMatch::connect(PatternDetectionParams {
            confidence_threshold: 0.0,
            patterns: None,
        })
        .await?;

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                let mut envelope = unwrap_envelope(item).await?;

                let text_spans = collect_text_spans(&envelope.document).await;
                if !text_spans.is_empty() {
                    let pm_ref = &pattern_match;
                    let do_pattern = || {
                        let spans = text_spans.clone();
                        let shared = self.shared.clone();
                        async move {
                            let input = ParallelContext::new(spans, shared);
                            pm_ref.call(input).await
                        }
                    };
                    let output = call_with_retry(retry, do_pattern).await?;
                    envelope.apply(output.into_inner());
                }

                count += 1;
                let envelope = Arc::new(envelope);
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_fusion(
        &self,
        node_id: Uuid,
        cfg: &graph::Fusion,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        if cfg.confidence_calibration {
            tracing::warn!(target: TARGET, "confidence_calibration requested but no calibration operation exists yet, skipping");
        }
        if cfg.contextual_adjustment {
            tracing::warn!(target: TARGET, "contextual_adjustment requested but no contextual adjustment operation exists yet, skipping");
        }

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                let mut envelope = unwrap_envelope(item).await?;

                if cfg.entity_deduplication && !envelope.entities.is_empty() {
                    let do_dedup = || {
                        let entities = envelope.entities.clone();
                        let shared = self.shared.clone();
                        async move {
                            let input = ParallelContext::new(entities, shared);
                            Deduplication.call(input).await
                        }
                    };
                    let output = call_with_retry(retry, do_dedup).await?;
                    envelope.apply(output.into_inner());
                }

                if !envelope.entities.is_empty() {
                    let do_ensemble = || {
                        let entities = envelope.entities.clone();
                        let shared = self.shared.clone();
                        async move {
                            let input = ParallelContext::new(entities, shared);
                            Ensemble::new(FusionStrategy::MaxConfidence)
                                .call(input)
                                .await
                        }
                    };
                    let output = call_with_retry(retry, do_ensemble).await?;
                    envelope.apply(output.into_inner());
                }

                count += 1;
                let envelope = Arc::new(envelope);
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_redaction(
        &self,
        node_id: Uuid,
        cfg: &graph::Redaction,
        retry: Option<&CompiledRetryPolicy>,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        let policies = &self.shared.policies;
        let rules = policies
            .policies
            .iter()
            .flat_map(|p| p.rules.clone())
            .collect();
        let eval = EvaluatePolicy::connect(EvaluatePolicyParams {
            rules,
            default_spec: nvisy_ontology::policy::Strategy::Text(
                nvisy_ontology::policy::TextStrategy::Mask { mask_char: '*' },
            ),
            default_confidence_threshold: 0.5,
        })
        .await?;

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                let mut envelope = unwrap_envelope(item).await?;

                if !envelope.entities.is_empty() {
                    let eval_ref = &eval;
                    let do_eval = || {
                        let entities = envelope.entities.clone();
                        let shared = self.shared.clone();
                        async move {
                            let input = ParallelContext::new(entities, shared);
                            eval_ref.call(input).await
                        }
                    };
                    let eval_output = call_with_retry(retry, do_eval).await?;
                    envelope.apply(eval_output.into_inner());
                }

                // Content-level redaction requires concrete handler types
                // (TxtHandler, PngHandler, WavHandler, CsvHandler) extracted
                // from the type-erased Document. The codec does not yet support
                // handler downcast, so the Redaction operation cannot be called.
                // Policy evaluation above already populated the audit decisions
                // and records. When the codec exposes a modality-agnostic
                // redaction API or handler downcast, content-level redaction
                // will be wired here.

                if cfg.validation {
                    tracing::warn!(target: TARGET, "redaction validation requested but Validation operation is not yet implemented, skipping");
                }
                if cfg.process_metadata {
                    tracing::debug!(target: TARGET, "metadata processing requested, currently handled by policy evaluation");
                }

                count += 1;
                let envelope = Arc::new(envelope);
                for tx in senders {
                    let _ = tx.send(Arc::clone(&envelope)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_context_passthrough(
        &self,
        node_id: Uuid,
        kind: &GraphNodeKind,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        tracing::debug!(target: TARGET, action = %kind, "context operation not yet implemented, passing through");

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                count += 1;
                for tx in senders {
                    let _ = tx.send(Arc::clone(&item)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    async fn execute_generate_context(
        &self,
        node_id: Uuid,
        cfg: &graph::GenerateContext,
        senders: &[mpsc::Sender<Arc<DocumentEnvelope>>],
        receivers: &mut [mpsc::Receiver<Arc<DocumentEnvelope>>],
    ) -> Result<NodeOutput, Error> {
        if cfg.summarization {
            tracing::warn!(target: TARGET, "context summarization requested but Summarization operation is not yet implemented, skipping");
        }
        if cfg.translation {
            tracing::warn!(target: TARGET, "context translation requested but Translation operation is not yet implemented, skipping");
        }
        if cfg.audit {
            tracing::debug!(target: TARGET, "context audit generation: audit records are already accumulated on the envelope");
        }

        let mut count = 0u64;
        for rx in receivers.iter_mut() {
            while let Some(item) = rx.recv().await {
                count += 1;
                for tx in senders {
                    let _ = tx.send(Arc::clone(&item)).await;
                }
            }
        }

        Ok(NodeOutput {
            node_id,
            items_processed: count,
            error: None,
            envelopes: Vec::new(),
        })
    }

    /// Build an [`OcrAgent`] from the LLM config section.
    fn build_ocr_agent(&self) -> Result<OcrAgent, Error> {
        let llm_section = self.config.llm.as_ref().ok_or_else(|| {
            Error::new(ErrorKind::Validation, "OCR verification requires an LLM provider")
        })?;
        let provider = llm_section.provider.as_ref().ok_or_else(|| {
            Error::new(ErrorKind::Validation, "OCR verification requires an LLM provider")
        })?;
        let agent_config = llm_section.policy.clone().unwrap_or_default();
        OcrAgent::new(provider, agent_config)
            .map_err(|e| Error::runtime(e.to_string(), "ocr-agent", false))
    }
}

/// Take ownership of the envelope from an `Arc`.
///
/// When the strong reference count is 1, this moves the envelope out
/// without copying. When the count is > 1 (fan-out), it encodes the
/// document to bytes and decodes a fresh copy — correct but expensive.
async fn unwrap_envelope(arc: Arc<DocumentEnvelope>) -> Result<DocumentEnvelope, Error> {
    match Arc::try_unwrap(arc) {
        Ok(envelope) => Ok(envelope),
        Err(arc) => {
            let content_data = arc.document.encode()?;
            let document = Document::decode(&content_data).await?;
            Ok(DocumentEnvelope {
                document,
                entities: arc.entities.clone(),
                audit: arc.audit.clone(),
            })
        }
    }
}

/// Call a closure with optional retry policy.
async fn call_with_retry<T, F, Fut>(
    retry: Option<&CompiledRetryPolicy>,
    mut f: F,
) -> Result<T, Error>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    match retry {
        Some(policy) => policy.with_retry(f).await,
        None => f().await,
    }
}

/// Collect text spans as `Vec<Span<TxtSpan, String>>` for NER.
async fn collect_ner_spans(
    doc: &Document,
) -> Vec<Span<nvisy_codec::handler::TxtSpan, String>> {
    match doc {
        Document::Text(h) => {
            let spans: Vec<_> = h.text_spans().await.collect().await;
            spans
                .into_iter()
                .map(|s| Span::new(nvisy_codec::handler::TxtSpan(s.id), s.data.into_inner()).with_source(s.source))
                .collect()
        }
        Document::Rich(h) => {
            let spans: Vec<_> = h.text_spans().await.collect().await;
            spans
                .into_iter()
                .map(|s| Span::new(nvisy_codec::handler::TxtSpan(s.id), s.data.into_inner()).with_source(s.source))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Collect text spans as `Vec<Span<usize, TextData>>` for pattern matching.
async fn collect_text_spans(
    doc: &Document,
) -> Vec<Span<usize, TextData>> {
    match doc {
        Document::Text(h) => h.text_spans().await.collect().await,
        Document::Rich(h) => h.text_spans().await.collect().await,
        _ => Vec::new(),
    }
}

/// Convert an OCR word to a detected entity.
///
/// OCR words carry extracted text but not entity-level classification.
/// Visual extraction produces raw text — entity detection is handled by
/// downstream NER/pattern nodes. Returns `None` since OCR words are not
/// entities by themselves.
fn word_to_entity(_word: &nvisy_ocr::Word) -> Option<nvisy_ontology::entity::Entity> {
    None
}
