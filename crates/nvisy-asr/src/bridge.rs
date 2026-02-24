//! [`TranscribeBackend`] implementation for [`PythonBridge`].

use serde_json::Value;

use nvisy_core::Error;
use nvisy_python::bridge::PythonBridge;
use nvisy_python::transcribe::TranscribeParams;

use crate::backend::{TranscribeBackend, TranscribeConfig};

/// Converts [`TranscribeConfig`] to [`TranscribeParams`] and delegates to
/// `nvisy_python::transcribe`.
#[async_trait::async_trait]
impl TranscribeBackend for PythonBridge {
    async fn transcribe(
        &self,
        audio_data: &[u8],
        mime_type: &str,
        config: &TranscribeConfig,
    ) -> Result<Vec<Value>, Error> {
        let params = TranscribeParams {
            language: config.language.clone(),
            enable_speaker_diarization: config.enable_speaker_diarization,
            confidence_threshold: config.confidence_threshold,
        };
        nvisy_python::transcribe::transcribe(self, audio_data, mime_type, &params).await
    }
}
