//! Type-erased STT extractor wrapping any [`SttBackend`].
//!
//! [`SttBackend`]: crate::backend::SttBackend

mod transcription;

use std::fmt;
use std::sync::Arc;

use nvisy_core::Result;
use nvisy_core::entity::ModelProvenance;
use nvisy_core::extraction::{Extractor, ExtractorOutput, Span};
use nvisy_core::modality::{Audio, AudioExtraction};
use tracing::instrument;

pub use self::transcription::Transcription;
use crate::backend::{SttBackend, SttRequest};

const TARGET: &str = "nvisy_stt::extraction";

/// Type-erased STT extractor wrapping any [`SttBackend`].
///
/// Owns an `Arc<dyn SttBackend>` and forwards transcription requests
/// to it, providing a concrete, object-safe entry point without
/// generics at every call site. Cloning shares the backend.
#[derive(Clone)]
pub struct SttExtractor {
    backend: Arc<dyn SttBackend>,
}

impl fmt::Debug for SttExtractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SttExtractor").finish_non_exhaustive()
    }
}

impl SttExtractor {
    /// Wrap any [`SttBackend`] implementation.
    pub fn new(backend: impl SttBackend) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Provenance of the wrapped backend, forwarded from
    /// [`SttBackend::provenance`].
    pub fn provenance(&self) -> ModelProvenance {
        self.backend.provenance()
    }
}

#[async_trait::async_trait]
impl Extractor<Audio> for SttExtractor {
    type Output = Transcription;

    #[instrument(target = TARGET, skip_all, fields(audio_bytes = span.data.bytes.len()))]
    async fn extract(&self, span: &Span<Audio>) -> Result<ExtractorOutput<Audio, Self::Output>> {
        let synthesized;
        let filename = match span.data.filename.as_deref() {
            Some(name) => name,
            None => {
                synthesized = format!("audio.{}", span.data.extension());
                synthesized.as_str()
            }
        };
        let request = SttRequest {
            audio: span.data.bytes.as_ref(),
            filename,
            language: span.language.as_ref(),
            correlation_id: span.correlation_id,
        };
        let response = self.backend.transcribe(request).await?;
        tracing::debug!(
            target: TARGET,
            segments = response.segments.len(),
            "transcription complete",
        );
        Ok(ExtractorOutput::new(
            Transcription::new(response.segments),
            AudioExtraction::Transcription(self.provenance()),
        ))
    }
}
