//! STT deployment configuration.
//!
//! Symmetric with [`super::ner`] and [`super::ocr`]: deployment
//! operator owns backend choice, connection details, and
//! (future) credentials. The request wire holds nothing about
//! STT: every audio-modality analyzer picks up the operator's
//! STT enricher automatically.
//!
//! ## Layout
//!
//! - [`SttConfig`] is the top-level bag: the enricher lineup.
//! - [`SttEnricherConfig`] declares one enricher instance: name
//!   (for the list-enrichers accessor) + backend selection with
//!   its per-kind fields flattened onto the wire.
//! - [`SttBackend`] is the discriminated backend enum: Bento
//!   today.

mod enricher;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::enricher::{SttBackend, SttEnricherConfig};

/// Top-level STT configuration. Loaded from the deployment's
/// `[stt]` config section.
///
/// Only one STT enricher attaches per audio analyzer today (an
/// elide constraint on `Enricher<Audio>`). The lineup shape
/// mirrors [`super::ner::NerConfig`] for wire symmetry; the
/// engine rejects `enrichers.len() > 1` at compile time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SttConfig {
    /// The enricher lineup. Empty means no STT wired; the
    /// audio-modality analyzer skips the enricher attach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enrichers: Vec<SttEnricherConfig>,
}
