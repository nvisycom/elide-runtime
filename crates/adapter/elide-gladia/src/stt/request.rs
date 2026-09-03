//! Outgoing request shaping for `POST /v2/pre-recorded`.
//!
//! The SDK builds the body; this decides what goes on it.

use gladia::model::DiarizationConfigDto;
use gladia::prerecorded::TranscriptionRequest;

/// Apply this backend's settings to a transcription request.
///
/// Gladia's own enrichment options (`pii_redaction`,
/// `named_entity_recognition`, summarization, sentiment) are deliberately
/// left off: redaction is elide's job, and asking the provider to do it too
/// would produce two disagreeing views of the same audio.
pub(super) fn configure(
    builder: TranscriptionRequest,
    diarize: bool,
    diarization: Option<DiarizationConfigDto>,
) -> TranscriptionRequest {
    match (diarize, diarization) {
        (true, Some(config)) => builder.with_diarization(config),
        (true, None) => builder.with_diarization_default(),
        (false, _) => builder,
    }
}
