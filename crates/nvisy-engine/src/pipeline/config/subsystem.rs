//! Provider subsystem configuration sections (OCR, LLM, STT, TTS).

use nvisy_ocr::{OcrProvider, RunParams as OcrRunParams};
use nvisy_rig::agent::{AgentConfig, AgentProvider};
use nvisy_rig::audio::{SttProvider, TtsProvider};
use serde::{Deserialize, Serialize};

/// OCR subsystem configuration.
///
/// Controls the optical character recognition provider and its runtime
/// parameters (confidence thresholds, language hints, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrSection {
    /// OCR provider selection and connection settings.
    pub provider: Option<OcrProvider>,
    /// OCR runtime parameters (confidence thresholds, etc.).
    pub policy: Option<OcrRunParams>,
}

/// LLM subsystem configuration.
///
/// Controls the language model provider used for NER, OCR verification,
/// and other inference tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmSection {
    /// LLM provider selection and connection settings.
    pub provider: Option<AgentProvider>,
    /// LLM sampling and retry parameters.
    pub policy: Option<AgentConfig>,
}

/// Speech-to-text subsystem configuration.
///
/// Controls the STT provider used by [`Extraction`]
/// nodes for audio transcription.
///
/// [`Extraction`]: crate::workflow::Extraction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SttSection {
    /// STT provider selection and connection settings.
    pub provider: Option<SttProvider>,
}

/// Text-to-speech subsystem configuration.
///
/// Controls the TTS provider for audio generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TtsSection {
    /// TTS provider selection and connection settings.
    pub provider: Option<TtsProvider>,
}
