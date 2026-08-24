//! OCR deployment configuration.
//!
//! Symmetric with [`super::ner`]: deployment operator owns
//! backend choice, connection details, and (future) credentials.
//! The request wire holds nothing about OCR: every image-modality
//! analyzer picks up the operator's OCR enricher automatically.
//!
//! ## Layout
//!
//! - [`OcrConfig`] is the top-level bag: the enricher lineup.
//! - [`Component<OcrBackend>`] declares one enricher instance: name
//!   (for the list-enrichers accessor) + backend selection with
//!   its per-kind fields flattened onto the wire.
//! - [`OcrBackend`] is the discriminated backend enum: Bento
//!   today.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::super::Component;

/// Top-level OCR configuration. Loaded from the deployment's
/// `[ocr]` config section.
///
/// Only one OCR enricher attaches per image analyzer today (an
/// elide constraint on `Enricher<Image>`). The lineup shape
/// mirrors [`NerConfig`](crate::NerConfig) for wire symmetry; the
/// engine rejects `enrichers.len() > 1` at compile time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OcrConfig {
    /// The enricher lineup. Empty means no OCR wired; the
    /// image-modality analyzer skips the enricher attach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enrichers: Vec<Component<OcrBackend>>,
}

/// How a configured enricher talks to its OCR backend.
///
/// One rig variant today (BentoML-hosted). The `Mock` variant
/// exists only when the consuming crate enables the `test-utils`
/// feature, so the wire rejects `kind = "mock"` in production
/// builds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OcrBackend {
    /// BentoML-hosted OCR service. Engine wires the shared
    /// `elide-bento` client; per-enricher URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// No-op backend; recognises no blocks. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}

impl super::super::Backend for OcrBackend {
    /// Provider slug for the list-enrichers accessor.
    fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "test-utils")]
            Self::Mock => "mock",
        }
    }
}
