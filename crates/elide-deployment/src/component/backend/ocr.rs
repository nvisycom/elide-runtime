//! OCR deployment configuration.
//!
//! Symmetric with the recognizer backends: the deployment operator
//! owns backend choice, connection details, and (future)
//! credentials. The request wire holds nothing about OCR: every
//! image-modality analyzer picks up the operator's OCR enricher
//! automatically.
//!
//! ## Layout
//!
//! - [`Enrichers`] is the top-level bag, holding this lineup beside
//!   the STT one.
//! - [`Component<OcrBackend>`] declares one enricher instance: name
//!   (for the list-enrichers accessor) + backend selection with its
//!   per-kind fields flattened onto the wire.
//! - [`OcrBackend`] is the discriminated backend enum: Bento today.
//!
//! [`Enrichers`]: super::Enrichers

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::component::Backend;

///
/// How a configured enricher talks to its OCR backend.
///
/// One rig variant today (BentoML-hosted). The `Mock` variant
/// exists only when the consuming crate enables the `mocks`
/// feature, so the wire rejects `kind = "mock"` in production
/// builds.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
// Denies unknown fields even though `Component` flattens this:
// the attribute is refused on the *outer* struct (the flattened
// fields reach it first and look unknown), but on the enum itself
// it catches a misspelled backend key -- `zero_retenton` for
// `zero_retention` -- which would otherwise be dropped, leaving a
// deployment believing it asked for something it did not.
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OcrBackend {
    /// BentoML-hosted OCR service. Engine wires the shared
    /// `elide-bentoml` client; per-enricher URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// Google Cloud Document AI.
    ///
    /// Credentials come from Application Default Credentials, so a
    /// deployment holds none here. The processor is the deployment's
    /// own: Document AI bills and versions per processor.
    #[cfg(feature = "google-document-ai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google-document-ai")))]
    GoogleDocumentAi {
        /// Full resource name of the processor to call, as
        /// `projects/<p>/locations/<l>/processors/<id>`.
        processor: String,
        /// Override the service endpoint, for a regional or private
        /// deployment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
    },
    /// No-op backend; recognises no blocks. Test-only.
    #[cfg(feature = "mocks")]
    #[cfg_attr(docsrs, doc(cfg(feature = "mocks")))]
    Mock,
}

impl OcrBackend {
    /// A BentoML-hosted OCR service at `base_url`, targeting
    /// `model`.
    pub fn bento(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Bento {
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

impl Backend for OcrBackend {
    /// Provider slug for the list-enrichers accessor.
    fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "google-document-ai")]
            Self::GoogleDocumentAi { .. } => "google/document-ai",
            #[cfg(feature = "mocks")]
            Self::Mock => "mock",
        }
    }
}
