//! Speech-to-text transcription via the Python backend.
//!
//! Calls `nvisy_ai.transcribe()` through the Python bridge to perform
//! speech transcription on audio, returning raw JSON values.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

use nvisy_core::Error;
use crate::bridge::{PythonBridge, from_pyerr};

/// Parameters for transcription, independent of any pipeline types.
#[derive(Debug, Clone)]
pub struct TranscribeParams {
    /// BCP-47 language tag for transcription.
    pub language: String,
    /// Whether to perform speaker diarization.
    pub enable_speaker_diarization: bool,
    /// Minimum confidence threshold for results.
    pub confidence_threshold: f64,
}

/// Call Python `transcribe()` synchronously via `spawn_blocking`.
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn transcribe(
    bridge: &PythonBridge,
    audio_data: &[u8],
    mime_type: &str,
    params: &TranscribeParams,
) -> Result<Vec<Value>, Error> {
    let audio_data = audio_data.to_vec();
    let mime_type = mime_type.to_string();
    let params = params.clone();

    bridge
        .call_sync("transcribe", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("audio_bytes", &audio_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("language", &params.language).map_err(from_pyerr)?;
            kwargs.set_item("enable_speaker_diarization", params.enable_speaker_diarization).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}

/// Call Python `transcribe()` as a **coroutine** (async Python function).
///
/// Returns raw JSON dicts — no domain-type construction.
pub async fn transcribe_async(
    bridge: &PythonBridge,
    audio_data: &[u8],
    mime_type: &str,
    params: &TranscribeParams,
) -> Result<Vec<Value>, Error> {
    let audio_data = audio_data.to_vec();
    let mime_type = mime_type.to_string();
    let params = params.clone();

    bridge
        .call_async("transcribe", move |py| {
            let kwargs = PyDict::new(py);
            kwargs.set_item("audio_bytes", &audio_data[..]).map_err(from_pyerr)?;
            kwargs.set_item("mime_type", &mime_type).map_err(from_pyerr)?;
            kwargs.set_item("language", &params.language).map_err(from_pyerr)?;
            kwargs.set_item("enable_speaker_diarization", params.enable_speaker_diarization).map_err(from_pyerr)?;
            kwargs.set_item("confidence_threshold", params.confidence_threshold).map_err(from_pyerr)?;
            Ok(kwargs)
        })
        .await
}
