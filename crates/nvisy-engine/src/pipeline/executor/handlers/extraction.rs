//! Extraction handlers: visual (OCR) and audial (STT).

use futures::StreamExt;
use nvisy_codec::handler::{Handler, ImageHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::{Error, ErrorKind};
use nvisy_http::HttpClient;
use nvisy_rig::agent::OcrAgent;
use nvisy_rig::audio::stt::{SttConfig, SttService};

use super::super::handler::NodeHandler;
use super::retry;
use crate::graph;
use crate::operation::inference::{Ocr, OcrVerification, OcrVerificationInput};
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};
use crate::pipeline::config::RuntimeConfig;
use crate::pipeline::policy::CompiledRetryPolicy;

const TARGET: &str = "nvisy_engine::pipeline::executor";

pub(crate) struct VisualExtractionHandler {
    ocr: Ocr,
    verifier: Option<OcrVerification>,
    shared: SharedContext,
    retry: Option<CompiledRetryPolicy>,
}

impl VisualExtractionHandler {
    pub fn new(
        cfg: &graph::VisualExtraction,
        config: &RuntimeConfig,
        http_client: &HttpClient,
        shared: SharedContext,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Self, Error> {
        let ocr_section = config.ocr.as_ref();
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
        let ocr_engine = ocr_provider.into_engine_with_client(http_client.clone());
        let ocr = Ocr::new(ocr_engine, ocr_params);

        let verifier = if cfg.verification {
            match build_ocr_agent(config) {
                Ok(agent) => Some(OcrVerification::new(agent)),
                Err(e) => {
                    tracing::warn!(target: TARGET, error = %e, "OCR verification requested but agent could not be built, skipping");
                    None
                }
            }
        } else {
            None
        };

        if cfg.entity_detection {
            tracing::warn!(target: TARGET, "entity_detection requested but CvProvider is not yet configurable, skipping");
        }

        Ok(Self { ocr, verifier, shared, retry })
    }
}

#[async_trait::async_trait]
impl NodeHandler for VisualExtractionHandler {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        if let Document::Image(ref handler) = envelope.document {
            let image_spans: Vec<_> = handler.image_spans().await.collect().await;
            let ocr_spans: Vec<Span<(), _>> = image_spans
                .into_iter()
                .map(|s| Span::new((), s.data).with_source(s.source))
                .collect();

            let ocr_ref = &self.ocr;
            let retry = self.retry.as_ref();
            let _ocr_output = retry::call(retry, || {
                let spans = ocr_spans.clone();
                let shared = self.shared.clone();
                async move {
                    let input = ParallelContext::new(spans, shared);
                    ocr_ref.call(input).await
                }
            })
            .await?;

            if let Some(ref verifier) = self.verifier
                && !envelope.entities.is_empty()
            {
                let verify_spans: Vec<_> = match &envelope.document {
                    Document::Image(h) => h
                        .image_spans()
                        .await
                        .collect::<Vec<_>>()
                        .await
                        .into_iter()
                        .map(|s| Span::new((), s.data).with_source(s.source))
                        .collect(),
                    _ => Vec::new(),
                };

                let do_verify = || {
                    let input = OcrVerificationInput {
                        image_spans: verify_spans.clone(),
                        entities: envelope.entities.clone(),
                    };
                    let shared = self.shared.clone();
                    async move {
                        let ctx = ParallelContext::new(input, shared);
                        verifier.call(ctx).await
                    }
                };
                match retry::call(retry, do_verify).await {
                    Ok(output) => envelope.apply(output.into_inner()),
                    Err(e) => tracing::warn!(
                        target: TARGET,
                        error = %e,
                        "OCR verification failed, keeping unverified entities"
                    ),
                }
            }
        }
        Ok(envelope)
    }
}

pub(crate) struct AudialExtractionHandler {
    stt: SttService,
    retry: Option<CompiledRetryPolicy>,
}

impl AudialExtractionHandler {
    pub fn new(
        cfg: &graph::AudialExtraction,
        config: &RuntimeConfig,
        http_client: &HttpClient,
        retry: Option<CompiledRetryPolicy>,
    ) -> Result<Self, Error> {
        let stt_provider = config
            .stt
            .as_ref()
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "audial_extraction requires an STT provider in config",
                )
            })?;

        let stt = SttService::with_http_client(&stt_provider, SttConfig::default(), http_client.clone())
            .map_err(|e: nvisy_rig::error::Error| Error::runtime(e.to_string(), "stt-service", false))?;

        if cfg.diarization {
            tracing::warn!(target: TARGET, "diarization requested but not yet supported, skipping");
        }

        Ok(Self { stt, retry })
    }
}

#[async_trait::async_trait]
impl NodeHandler for AudialExtractionHandler {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        if let Document::Audio(ref handler) = envelope.document {
            let audio_data = Handler::encode(handler)?;
            let filename: String = audio_data
                .filename
                .as_deref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "audio.wav".to_string());

            let stt_ref = &self.stt;
            let do_transcribe = || {
                let bytes = audio_data.as_bytes().to_vec();
                let fname = filename.clone();
                async move {
                    stt_ref
                        .transcribe(&bytes, &fname)
                        .await
                        .map_err(|e: nvisy_rig::error::Error| {
                            Error::runtime(e.to_string(), "stt-transcribe", e.is_retryable())
                        })
                }
            };
            let stt_output = retry::call(self.retry.as_ref(), do_transcribe).await?;

            tracing::debug!(
                target: TARGET,
                text_len = stt_output.text.len(),
                "transcription complete for audio document"
            );
            // TODO: inject transcribed text into envelope when it gains a
            // `transcriptions` field for downstream NER.
        }
        Ok(envelope)
    }
}

fn build_ocr_agent(config: &RuntimeConfig) -> Result<OcrAgent, Error> {
    let llm_section = config.llm.as_ref().ok_or_else(|| {
        Error::new(ErrorKind::Validation, "OCR verification requires an LLM provider")
    })?;
    let provider = llm_section.provider.as_ref().ok_or_else(|| {
        Error::new(ErrorKind::Validation, "OCR verification requires an LLM provider")
    })?;
    let agent_config = llm_section.policy.clone().unwrap_or_default();
    OcrAgent::new(provider, agent_config)
        .map_err(|e| Error::runtime(e.to_string(), "ocr-agent", false))
}
