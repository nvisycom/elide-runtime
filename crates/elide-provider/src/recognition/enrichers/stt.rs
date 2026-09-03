//! STT deployment configuration.
//!
//! Symmetric with the recognizer backends: the deployment operator
//! owns backend choice, connection details, and (future)
//! credentials. The request wire holds nothing about STT: every
//! audio-modality analyzer picks up the operator's STT enricher
//! automatically.
//!
//! ## Layout
//!
//! - [`Enrichers`] is the top-level bag, holding this lineup beside
//!   the OCR one.
//! - [`Component<SttBackend>`] declares one enricher instance: name
//!   (for the list-enrichers accessor) + backend selection with its
//!   per-kind fields flattened onto the wire.
//! - [`SttBackend`] is the discriminated backend enum: a
//!   self-hosted BentoML service, or Gladia's hosted API under the
//!   `gladia` feature.
//!
//! [`Enrichers`]: super::Enrichers

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

///
/// How a configured enricher talks to its STT backend.
///
/// Self-hosted BentoML, or Gladia's hosted API when the consuming
/// crate enables the `gladia` feature. The `Mock` variant exists
/// only under `test-utils`, so the wire rejects `kind = "mock"` in
/// production builds.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttBackend {
    /// BentoML-hosted STT service. Engine wires the shared
    /// `elide-bentoml` client; per-enricher URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// Gladia's hosted transcription API.
    ///
    /// # Audio leaves your infrastructure
    ///
    /// Gladia is a third-party service, and this enricher runs
    /// *before* redaction: it uploads the audio as received. Voice
    /// is biometric personal data, so for a recording carrying PHI
    /// or PII this is a disclosure to a processor, whatever the
    /// pipeline does downstream. Whether that is acceptable is a
    /// deployment policy question. Prefer [`Bento`] when it is not.
    ///
    /// A run that used this backend says so: its enricher stamps a
    /// `gladia` model event onto the audit's usage, so a trail
    /// shows which transcriber saw the audio.
    ///
    /// [`Bento`]: Self::Bento
    #[cfg(feature = "gladia")]
    #[cfg_attr(docsrs, doc(cfg(feature = "gladia")))]
    Gladia {
        /// Gladia API key. Held in the deployment's own config,
        /// never on the request wire.
        ///
        /// Read from configuration but never written back: the
        /// config type is `Serialize`, and a host that dumps its
        /// effective settings — a debug endpoint, a startup log, a
        /// crash report — would otherwise put the key in plaintext
        /// wherever that lands. Serializing a config therefore
        /// yields one that cannot be loaded as-is, which is the
        /// intended trade: a secret is supplied, not round-tripped.
        #[serde(skip_serializing)]
        api_key: String,
        /// Base URL of the Gladia API, overriding the SDK's
        /// default (`https://api.gladia.io`).
        ///
        /// For a regional endpoint, or to point a test deployment
        /// at a local stand-in rather than the live service. Unlike
        /// the key this is not a secret, so it round-trips.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
    },
    /// No-op backend; emits no segments. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}

/// Hand-written so [`Gladia`](Self::Gladia)'s API key never reaches
/// a log. Everything else prints as the derive would.
impl fmt::Debug for SttBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bento { base_url, model } => f
                .debug_struct("Bento")
                .field("base_url", base_url)
                .field("model", model)
                .finish(),
            #[cfg(feature = "gladia")]
            Self::Gladia { base_url, .. } => f
                .debug_struct("Gladia")
                .field("api_key", &"***")
                .field("base_url", base_url)
                .finish(),
            #[cfg(feature = "test-utils")]
            Self::Mock => f.write_str("Mock"),
        }
    }
}

impl super::super::Backend for SttBackend {
    /// Provider slug for the list-enrichers accessor.
    fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "gladia")]
            Self::Gladia { .. } => "gladia",
            #[cfg(feature = "test-utils")]
            Self::Mock => "mock",
        }
    }
}

#[cfg(all(test, feature = "gladia"))]
mod tests {
    use super::SttBackend;

    /// The key must not escape through *either* rendering: the
    /// hand-written `Debug` covers logs and panics, `skip_serializing`
    /// covers a host dumping its effective config. Restoring
    /// `#[derive(Debug)]` or dropping the attribute would silently
    /// undo one, so both are pinned here rather than left to review.
    #[test]
    fn the_gladia_api_key_never_escapes() {
        const SECRET: &str = "sk-super-secret-value";
        let backend = SttBackend::Gladia {
            api_key: SECRET.to_owned(),
            base_url: Some("https://eu-west.gladia.io".to_owned()),
        };

        let debugged = format!("{backend:?}");
        assert!(
            !debugged.contains(SECRET),
            "the API key reached a Debug rendering: {debugged}",
        );
        assert!(debugged.contains("***"), "expected a redaction: {debugged}");

        let json = serde_json::to_string(&backend).expect("the config serializes");
        assert!(
            !json.contains(SECRET),
            "the API key reached a serialized config: {json}",
        );
        assert!(
            json.contains("eu-west.gladia.io"),
            "but the base URL is not a secret and round-trips: {json}",
        );
    }
}
