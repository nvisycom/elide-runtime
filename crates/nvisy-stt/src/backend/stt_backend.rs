//! [`SttBackend`]: the unified per-call speech-to-text backend trait.
//!
//! One trait covers every flavour of provider — hosted APIs that emit
//! a single full-clip segment (OpenAI Whisper), hosted APIs that emit
//! diarized multi-speaker segments (Deepgram, AssemblyAI), and
//! local/self-hosted inference services. Each backend turns a
//! request (audio bytes + optional language hint) into a response
//! (an ordered list of [`TranscribedSegment`]).
//!
//! Object-safe: extractors hold `Arc<dyn SttBackend>` and dispatch
//! per call.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::entity::ModelProvenance;
use nvisy_core::primitive::LanguageTag;
use uuid::Uuid;

use super::transcribed_segment::TranscribedSegment;

/// One per-call STT request handed to an [`SttBackend`].
#[derive(Debug, Clone)]
pub struct SttRequest<'a> {
    /// Raw audio bytes (MP3, WAV, FLAC, etc.). The backend is
    /// responsible for honouring whatever container/codec it accepts.
    pub audio: &'a [u8],
    /// Original filename. Some backends use the extension for
    /// MIME-type detection on multipart uploads.
    pub filename: &'a str,
    /// Caller-asserted language. Backends that support per-call
    /// language hinting use this; others ignore it.
    pub language: Option<&'a LanguageTag>,
    /// Correlation UUID for tracing.
    pub correlation_id: Option<Uuid>,
}

/// One per-call STT response from an [`SttBackend`].
///
/// Wraps the segments the backend produced in their original order.
#[derive(Debug, Clone, Default)]
pub struct SttResponse {
    /// Segments predicted for the request, in backend order.
    pub segments: Vec<TranscribedSegment>,
}

impl SttResponse {
    /// Construct a response from segments.
    #[must_use]
    pub fn new(segments: Vec<TranscribedSegment>) -> Self {
        Self { segments }
    }
}

/// Per-call speech-to-text backend.
///
/// Implemented by everything that turns `(audio, language?)` into
/// transcribed segments — hosted provider clients (OpenAI Whisper,
/// Deepgram, AssemblyAI), local model wrappers, and the in-process
/// no-op test stub.
#[async_trait]
pub trait SttBackend: Send + Sync + 'static {
    /// Backend identity (model / service name + provenance kind).
    ///
    /// The document-side extraction phase reads this after STT runs and
    /// stamps it into [`AudioExtraction::Transcription`] on the
    /// document's metadata, so the audit records *which* STT pass
    /// produced the document.
    ///
    /// [`AudioExtraction::Transcription`]: nvisy_core::modality::AudioExtraction::Transcription
    fn provenance(&self) -> ModelProvenance;

    /// Transcribe `request`.
    ///
    /// # Errors
    ///
    /// Returns the underlying transport / parse / inference error.
    async fn transcribe(&self, request: SttRequest<'_>) -> Result<SttResponse>;
}
