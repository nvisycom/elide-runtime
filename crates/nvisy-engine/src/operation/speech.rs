//! Audial extraction: speech-to-text transcription.

use nvisy_codec::Document;
use nvisy_codec::handler::Handler;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_http::HttpClient;
use nvisy_rig::audio::stt::{SttConfig, SttOutput, SttService};

use crate::graph::RetryPolicy;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::audial_extraction";

/// Audial extraction: transcribes audio documents via STT.
pub struct AudialExtraction {
    stt: SttService,
    retry: Option<RetryPolicy>,
}

impl AudialExtraction {
    pub fn connect(
        cfg: &crate::graph::AudialExtraction,
        config: &RuntimeConfig,
        http_client: &HttpClient,
        retry: Option<RetryPolicy>,
    ) -> Result<Self> {
        let stt_provider = config
            .stt
            .as_ref()
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "audial_extraction requires an STT provider",
                )
            })?;

        let stt =
            SttService::with_http_client(&stt_provider, SttConfig::default(), http_client.clone())
                .map_err(|e: nvisy_rig::error::Error| {
                    Error::runtime(e.to_string(), "stt-service", false)
                })?;

        if cfg.diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

        Ok(Self { stt, retry })
    }

    pub(crate) async fn process(
        &self,
        envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        let Document::Audio(ref handler) = envelope.document else {
            return Ok(envelope);
        };

        let audio_data = Handler::encode(handler)?;
        let filename: String = audio_data
            .filename
            .as_deref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio.wav".to_string());

        let stt_ref = &self.stt;
        let retry = self.retry.as_ref();
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

        let stt_output = match retry {
            Some(policy) => policy.with_retry(do_transcribe).await?,
            None => do_transcribe().await?,
        };

        tracing::debug!(
            target: TARGET,
            text_len = stt_output.text.len(),
            "transcription complete",
        );
        // TODO: inject transcribed text into envelope for downstream NER.

        Ok(envelope)
    }
}

/// Typed input for the [`Operation`] impl: raw audio bytes + filename.
pub struct AudioInput {
    /// Raw audio bytes (WAV, MP3, etc.).
    pub audio_data: Vec<u8>,
    /// Original filename for MIME-type detection.
    pub filename: String,
}

impl Operation for AudialExtraction {
    type Input = ParallelContext<AudioInput>;
    type Output = ParallelContext<SttOutput>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|data| async move {
                self.stt
                    .transcribe(&data.audio_data, &data.filename)
                    .await
                    .map_err(|e: nvisy_rig::error::Error| {
                        Error::runtime(e.to_string(), "stt-transcribe", e.is_retryable())
                    })
            })
            .await
    }
}
