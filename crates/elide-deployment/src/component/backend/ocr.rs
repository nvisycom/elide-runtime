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

///
/// How a configured enricher talks to its OCR backend.
///
/// One rig variant today (BentoML-hosted). The `Mock` variant
/// exists only when the consuming crate enables the `mocks`
/// feature, so the wire rejects `kind = "mock"` in production
/// builds.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
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
    /// AWS Textract's `DetectDocumentText`.
    ///
    /// Credentials come from the AWS SDK's own chain — IAM role,
    /// instance metadata, profile, environment — unless this names
    /// them, so the usual deployment holds none here.
    #[cfg(feature = "aws-textract")]
    #[cfg_attr(docsrs, doc(cfg(feature = "aws-textract")))]
    AwsTextract {
        /// The AWS region to call.
        ///
        /// Absent resolves from the SDK's chain, the same as the
        /// credentials.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        /// Static credentials, for a deployment that cannot use the
        /// SDK's chain.
        ///
        /// Prefer an IAM role: these sit in the deployment file (or
        /// its `${VAR}` references) and must be rotated by hand.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        credentials: Option<AwsCredentials>,
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

/// Static AWS credentials, for a deployment that cannot use the
/// SDK's own credential chain.
///
/// Carrying these in configuration means rotating them by hand and
/// keeping the file wherever the deployment keeps secrets. An IAM
/// role or instance profile avoids both, and is what
/// [`AwsTextract`](OcrBackend::AwsTextract) uses when this is absent.
#[cfg(feature = "aws-textract")]
#[cfg_attr(docsrs, doc(cfg(feature = "aws-textract")))]
#[derive(Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AwsCredentials {
    /// The access key id.
    ///
    /// Read from configuration but never written back, like the
    /// secret beside it: it names the IAM principal, and a config
    /// dump is not where a deployment should disclose which account
    /// it calls as.
    #[serde(skip_serializing)]
    pub access_key_id: String,
    /// The secret access key.
    ///
    /// Read from configuration but never written back, as
    /// [`SttBackend::Deepgram`](super::SttBackend)'s key is: this
    /// type is `Serialize`, and a host dumping its effective
    /// settings would otherwise put the secret in plaintext
    /// wherever that dump lands.
    #[serde(skip_serializing)]
    pub secret_access_key: String,
    /// A session token, for temporary credentials.
    ///
    /// Not written back either; it is a credential in its own
    /// right.
    #[serde(default, skip_serializing)]
    pub session_token: Option<String>,
}

#[cfg(feature = "aws-textract")]
impl std::fmt::Debug for AwsCredentials {
    /// Redacts all three fields.
    ///
    /// Hand-written for the reason `SttBackend`'s is: a derived
    /// `Debug` would put the secret access key in every log line
    /// that formats a deployment.
    ///
    /// The access key id is redacted too. It is not secret on its
    /// own, but it names the IAM principal, and a debug line is not
    /// where a deployment should learn which account it is calling
    /// as. What such a line needs is whether credentials were
    /// configured at all, which the `***` conveys.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsCredentials")
            .field("access_key_id", &"***")
            .field("secret_access_key", &"***")
            .field("session_token", &self.session_token.as_ref().map(|_| "***"))
            .finish()
    }
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

impl crate::component::Backend for OcrBackend {
    /// Provider slug for the list-enrichers accessor.
    fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "aws-textract")]
            Self::AwsTextract { .. } => "aws/textract",
            #[cfg(feature = "google-document-ai")]
            Self::GoogleDocumentAi { .. } => "google/document-ai",
            #[cfg(feature = "mocks")]
            Self::Mock => "mock",
        }
    }
}
