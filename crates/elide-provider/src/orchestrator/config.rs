//! [`ProviderConfig`]: what a deployment decides once, as data.
//!
//! The serializable half of a [`Provider`]. A host reads it from a
//! file, an environment variable, or an encrypted row in its own
//! database, and builds; the provider that comes out knows nothing
//! about where it came from.
//!
//! [`Provider`]: super::Provider

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Provider;
use crate::recognition::{Enrichers, Recognizers};

/// Everything a deployment decides once, at startup.
///
/// Every field defaults to empty, so a config naming nothing builds
/// a provider that runs the pattern recognizers elide ships and no
/// model-backed ones.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderConfig {
    /// The recognizer lineups: which components find entities.
    pub recognizers: Recognizers,
    /// The enricher lineups: which components produce the context
    /// recognizers read.
    pub enrichers: Enrichers,
}

impl ProviderConfig {
    /// Build the provider this config describes.
    #[must_use]
    pub fn build(self) -> Provider {
        Provider::from_parts(self.recognizers, self.enrichers)
    }
}
