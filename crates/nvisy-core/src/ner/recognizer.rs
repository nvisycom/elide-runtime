//! [`NerRecognizer`]: one entry in the deployment's NER lineup.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One deployment-configured NER recognizer. Every entry in
/// [`NerConfig::recognizers`](super::NerConfig::recognizers)
/// runs when the request toggles `recognizers.ner = true`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerRecognizer {
    /// Recognizer name. Surfaces on the per-entity provenance
    /// trail so audits can attribute detections to a specific
    /// configured recognizer. Must be unique across the
    /// deployment's NER lineup.
    pub name: String,
    /// Backend selection + its per-kind fields, flattened onto
    /// the recognizer's wire shape.
    #[serde(flatten)]
    pub backend: NerBackendConfig,
}

/// How a configured recognizer talks to its NER backend.
///
/// One rig variant today (BentoML-hosted). Extensible with
/// authenticated variants (managed NER services) later; the
/// `Mock` variant exists only when the consuming crate enables
/// the `test-utils` feature, so the wire rejects
/// `kind = "mock"` in production builds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum NerBackendConfig {
    /// BentoML-hosted NER service. Engine wires the shared
    /// `elide-bento` client; per-recognizer URL + model come
    /// from this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// No-op backend; emits no entities. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}
