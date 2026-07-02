//! NER deployment configuration.
//!
//! The wire's `RecognizerParams.ner` is only a boolean; every
//! detail about which NER recognizer(s) actually run lives here,
//! on the deployment's side. Symmetric with [`crate::llm`]:
//! deployment operator owns model choice, connection details,
//! and (future) credentials; the wire only opts in or out.
//!
//! ## Layout
//!
//! - [`NerConfig`] is the top-level bag: the recognizer lineup.
//! - [`NerRecognizer`] declares one recognizer instance:
//!   name (for provenance) + backend selection with its
//!   per-kind fields flattened onto the wire.
//! - [`NerBackendConfig`] is the discriminated backend enum:
//!   Bento today, extensible with authenticated variants later.

mod recognizer;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::recognizer::{NerBackendConfig, NerRecognizer};

/// Top-level NER configuration. Loaded from the deployment's
/// `[ner]` config section.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerConfig {
    /// The recognizer lineup. Every entry runs when the request
    /// toggles `recognizers.ner = true`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognizers: Vec<NerRecognizer>,
}
