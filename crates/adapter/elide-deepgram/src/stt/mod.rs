//! [`DeepgramStt`]: an [`SttBackend`] backed by Deepgram's hosted
//! transcription API.
//!
//! [`SttBackend`]: elide_audio::stt::SttBackend

mod request;
mod response;

use async_trait::async_trait;
use deepgram::Deepgram;
use deepgram::common::audio_source::AudioSource;
use deepgram::common::options::Model;
use elide_audio::stt::{SttBackend, SttRequest, SttResponse};
use elide_core::Result;
use elide_core::entity::audit::ModelEvent;
use hipstr::HipStr;

use crate::error::DeepgramBackendError;

/// An [`SttBackend`] backed by Deepgram's hosted transcription API.
///
/// # Where the audio goes
///
/// Unlike the self-hosted backends, **this sends raw audio to a third
/// party**. In a redaction pipeline that means un-redacted audio leaves
/// your infrastructure before anything is redacted — personal data, and
/// biometric data under GDPR Article 4(14) where it is processed to
/// identify or authenticate a speaker. Whether that is acceptable is a
/// deployment policy question, not a technical one; see the crate README.
///
/// # Diarization
///
/// Off unless [`with_diarization`] is called. A segment carries the label
/// Deepgram assigned its utterance. Deepgram also labels each word, but
/// elide models speaker attribution on the segment alone, so those are
/// dropped rather than surfaced.
///
/// [`with_diarization`]: Self::with_diarization
#[derive(Debug, Clone)]
pub struct DeepgramStt {
    client: Deepgram,
    model: Model,
    diarize: bool,
}

impl DeepgramStt {
    /// Build from an API key, against Deepgram's public endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the client cannot be built — a malformed key.
    pub fn new(api_key: impl AsRef<str>) -> Result<Self> {
        Ok(Self::from_client(
            Deepgram::new(api_key).map_err(DeepgramBackendError::from)?,
        ))
    }

    /// Build from a pre-configured [`Deepgram`] client.
    ///
    /// For a regional or self-hosted endpoint. The SDK is re-exported as
    /// [`deepgram`], so a caller configuring the client does not need to
    /// depend on it separately:
    ///
    /// ```no_run
    /// use elide_deepgram::{DeepgramStt, deepgram};
    ///
    /// # fn main() -> elide_core::Result<()> {
    /// let client = deepgram::Deepgram::with_base_url_and_api_key(
    ///     "https://api.eu.deepgram.com",
    ///     std::env::var("DEEPGRAM_API_KEY").unwrap(),
    /// )
    /// .expect("client");
    /// let backend = DeepgramStt::from_client(client);
    /// # let _ = backend;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn from_client(client: Deepgram) -> Self {
        Self {
            client,
            model: Model::Nova3,
            diarize: false,
        }
    }

    /// Transcribe with a specific model rather than the default.
    ///
    /// Defaults to [`Model::Nova3`], Deepgram's current general model.
    #[must_use]
    pub fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    /// Request speaker labels.
    ///
    /// Deepgram detects the speaker count itself; there is nothing to bound.
    #[must_use]
    pub fn with_diarization(mut self) -> Self {
        self.diarize = true;
        self
    }
}

#[async_trait]
impl SttBackend for DeepgramStt {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: HipStr::borrowed("deepgram"),
            // The model is the deployment's choice and is worth recording:
            // Nova-3 and Whisper-via-Deepgram are different transcribers.
            version: Some(HipStr::from(self.model.as_ref().to_owned())),
            contextual: false,
        }
    }

    async fn transcribe(&self, request: SttRequest<'_>) -> Result<SttResponse> {
        let options = self::request::options(
            &self.model,
            self.diarize,
            request.language.map(ToString::to_string).as_deref(),
        );
        // Synchronous REST: one call returns the transcript, which is what
        // the `SttBackend` contract's single await wants.
        let source = AudioSource::from_buffer(request.audio.to_vec());
        let response = self
            .client
            .transcription()
            .prerecorded(source, &options)
            .await
            .map_err(DeepgramBackendError::from)?;

        self::response::decode(response).map_err(elide_core::Error::from)
    }
}
