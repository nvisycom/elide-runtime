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
//! - [`NerRecognizer`] declares one recognizer instance:
//!   name (for provenance) + backend selection with its
//!   per-kind fields flattened onto the wire.
//! - [`NerBackend`] is the discriminated backend enum:
//!   Bento today, extensible with authenticated variants later.

mod recognizer;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::recognizer::{NerBackend, NerRecognizer};

/// Top-level NER configuration. Loaded from the deployment's
/// `[ner]` config section.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerConfig {
    /// The recognizer lineup. Each entry runs when the request's
    /// `recognizers.ner` selects it (by allowlist name or by
    /// running the whole lineup).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognizers: Vec<NerRecognizer>,
}
