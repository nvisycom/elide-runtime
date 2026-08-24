//! NER deployment configuration.
//!
//! The wire's `RecognizerParams.ner` selects recognizers by
//! name (or the whole lineup); every detail about which NER
//! recognizer(s) actually run lives here, on the deployment's
//! side. Symmetric with [`super::llm`]: deployment operator
//! owns model choice, connection details, and (future)
//! credentials; the wire only names which of the operator's
//! recognizers to run.
//!
//! ## Layout
//!
//! - [`NerConfig`] is the top-level bag: the recognizer lineup.
//! - [`Component<NerBackend>`] declares one recognizer instance:
//!   name (for provenance) + backend selection with its
//!   per-kind fields flattened onto the wire.
//! - [`NerBackend`] is the discriminated backend enum:
//!   Bento today, extensible with authenticated variants later.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::super::Component;

/// Top-level NER configuration. Loaded from the deployment's
/// `[ner]` config section.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerConfig {
    /// The recognizer lineup. Each entry runs when the request's
    /// `recognizers.ner` selects it (by allowlist name or by
    /// running the whole lineup).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognizers: Vec<Component<NerBackend>>,
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
pub enum NerBackend {
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

impl super::super::Backend for NerBackend {
    /// Provider slug for the list-recognizers accessor.
    fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "test-utils")]
            Self::Mock => "mock",
        }
    }
}
