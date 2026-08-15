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
//! - [`OcrEnricherConfig`] declares one enricher instance: name
//!   (for the list-enrichers accessor) + backend selection with
//!   its per-kind fields flattened onto the wire.
//! - [`OcrBackend`] is the discriminated backend enum: Bento
//!   today.

mod enricher;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::enricher::{OcrBackend, OcrEnricherConfig};

/// Top-level OCR configuration. Loaded from the deployment's
/// `[ocr]` config section.
///
/// Only one OCR enricher attaches per image analyzer today (an
/// elide constraint on `Enricher<Image>`). The lineup shape
/// mirrors [`super::ner::NerConfig`] for wire symmetry; the
/// engine rejects `enrichers.len() > 1` at compile time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OcrConfig {
    /// The enricher lineup. Empty means no OCR wired; the
    /// image-modality analyzer skips the enricher attach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enrichers: Vec<OcrEnricherConfig>,
}
