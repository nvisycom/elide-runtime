//! [`SttEnricherConfig`]: one entry in the deployment's STT
//! enricher lineup, with its backend.

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One entry in the deployment's STT enricher lineup.
///
/// Only one STT enricher attaches per audio analyzer today (an
/// elide constraint on `Enricher<Audio>`); the wire keeps a
/// `Vec` for symmetry with recognizer lineups, and the engine
/// rejects `enrichers.len() != 1` at compile time.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SttEnricherConfig {
    /// Enricher name. Surfaces on the list-enrichers accessor so
    /// operators and SDK callers can identify what's wired.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional human-readable description. Also surfaces on the
    /// list-enrichers accessor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// Backend selection + its per-kind fields, flattened onto
    /// the enricher's wire shape.
    #[serde(flatten)]
    pub backend: SttBackend,
}

/// How a configured enricher talks to its STT backend.
///
/// One rig variant today (BentoML-hosted). The `Mock` variant
/// exists only when the consuming crate enables the `test-utils`
/// feature, so the wire rejects `kind = "mock"` in production
/// builds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SttBackend {
    /// BentoML-hosted STT service. Engine wires the shared
    /// `elide-bento` client; per-enricher URL + model come from
    /// this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// No-op backend; emits no segments. Test-only.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    Mock,
}

impl SttBackend {
    /// Provider slug for the list-enrichers accessor.
    #[must_use]
    pub fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "test-utils")]
            Self::Mock => "mock",
        }
    }
}
