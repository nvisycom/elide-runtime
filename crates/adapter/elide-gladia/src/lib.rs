#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod error;
mod response;

use async_trait::async_trait;
use elide_core::Result;
use elide_core::entity::audit::ModelEvent;
use elide_stt::{SttBackend, SttRequest, SttResponse};
use gladia::Client;
use gladia::model::DiarizationConfigDto;
use hipstr::HipStr;

use crate::error::GladiaError;

/// Model identifier reported in provenance.
///
/// Gladia does not name the model behind its API, so this identifies the
/// provider rather than a specific set of weights — the honest thing to
/// record when the provider can change them underneath us.
const MODEL_ID: &str = "gladia";

/// An [`SttBackend`] backed by Gladia's hosted transcription API.
///
/// # Where the audio goes
///
/// Unlike the self-hosted backends, **this sends raw audio to a third
/// party**. In a redaction pipeline that means un-redacted audio — voice
/// being biometric personal data — leaves your infrastructure before
/// anything is redacted. Whether that is acceptable is a deployment policy
/// question, not a technical one; see the crate README.
///
/// # Diarization
///
/// Off unless [`with_diarization`] is called. Gladia labels speakers per
/// **utterance**, not per word, so a segment is attributed as a whole and a
/// speaker change mid-utterance is not represented.
///
/// [`with_diarization`]: Self::with_diarization
#[derive(Debug, Clone)]
pub struct GladiaStt {
    client: Client,
    diarization: Option<DiarizationConfigDto>,
    diarize: bool,
}

impl GladiaStt {
    /// Build from an API key, against Gladia's public endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error when the client cannot be built — a malformed key
    /// or base URL.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::from_client(
            Client::builder()
                .with_api_key(api_key)
                .build()
                .map_err(GladiaError::from)?,
        )
    }

    /// Build from a pre-configured [`Client`].
    ///
    /// For a caller that needs to set a regional base URL, timeouts, retries
    /// or headers the SDK exposes but this backend does not re-export.
    ///
    /// # Errors
    ///
    /// Currently infallible; returns [`Result`] so future validation does
    /// not become a breaking change.
    pub fn from_client(client: Client) -> Result<Self> {
        Ok(Self {
            client,
            diarization: None,
            diarize: false,
        })
    }

    /// Request speaker labels.
    ///
    /// `config` bounds the speaker search; `None` lets Gladia decide, which
    /// is the right default unless the caller genuinely knows the count.
    #[must_use]
    pub fn with_diarization(mut self, config: Option<DiarizationConfigDto>) -> Self {
        self.diarize = true;
        self.diarization = config;
        self
    }
}

#[async_trait]
impl SttBackend for GladiaStt {
    fn provenance(&self) -> ModelEvent {
        ModelEvent {
            name: HipStr::borrowed(MODEL_ID),
            version: None,
            contextual: false,
        }
    }

    async fn transcribe(&self, request: SttRequest<'_>) -> Result<SttResponse> {
        // Gladia detects the container from the upload's filename, so a
        // caller-supplied name is forwarded; the fallback is only a label.
        let filename = request.filename.unwrap_or("audio").to_owned();
        let audio = request.audio.to_vec();
        let diarize = self.diarize;
        let diarization = self.diarization.clone();

        // One call covers upload, submit and poll: the `SttBackend` contract
        // is a single await, and the SDK owns the job's lifecycle.
        let response = self
            .client
            .prerecorded()
            .transcribe_file(filename, audio, |builder| {
                let builder = match (diarize, diarization) {
                    (true, Some(config)) => builder.with_diarization(config),
                    (true, None) => builder.with_diarization_default(),
                    (false, _) => builder,
                };
                // Gladia's own enrichment (pii_redaction, NER, summarization)
                // is deliberately left off: redaction is elide's job, and two
                // providers redacting the same audio would produce two
                // disagreeing views of it.
                builder
            })
            .await
            .map_err(GladiaError::from)?;

        response::decode(response).map_err(elide_core::Error::from)
    }
}
