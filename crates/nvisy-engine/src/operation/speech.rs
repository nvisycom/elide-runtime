//! Audial extraction operation.

//!
//! Runs at **phase 1**, after ingestion. Transcribes speech audio into
//! text using automatic speech recognition.

use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::workflow::AudialExtraction;
use nvisy_provider::audio::stt::{SttConfig, SttOutput, SttService};
use nvisy_provider::http::HttpClient;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::speech";

/// Audial extraction: transcribes audio documents via STT.
pub struct AudialExtractionOp {
    stt: SttService,
}

impl AudialExtractionOp {
    pub fn new(
        cfg: &AudialExtraction,
        config: &RuntimeConfig,
        http_client: &HttpClient,
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

        let stt = SttService::new(
            &stt_provider,
            SttConfig::default(),
            Some(http_client.clone()),
        )?;

        if cfg.diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

        Ok(Self { stt })
    }
}

/// Typed input for the [`Operation`] impl: raw audio bytes + filename.
pub struct AudioInput {
    /// Raw audio bytes (WAV, MP3, etc.).
    pub audio_data: Vec<u8>,
    /// Original filename for MIME-type detection.
    pub filename: String,
}

impl Operation for AudialExtractionOp {
    type Input = ParallelContext<AudioInput>;
    type Output = ParallelContext<SttOutput>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|data| async move {
                self.stt.transcribe(&data.audio_data, &data.filename).await
            })
            .await
    }
}
