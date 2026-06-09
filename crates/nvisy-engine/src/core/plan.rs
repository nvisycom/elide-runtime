//! Per-request bundle of per-phase configs the pipeline executes
//! against each imported document.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::pipeline::{DeduplicationParams, Detection, Extraction, Redaction, Validation};

/// Per-request bundle of per-phase configs.
///
/// The pipeline reads this once per document and routes each phase
/// to the matching field. Order of execution is fixed
/// (extraction → detection → dedup → redaction → validation); fields
/// here are configuration only, not sequencing.
#[derive(Debug, Clone, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Plan {
    /// Extraction settings per modality.
    pub extraction: Extraction,
    /// Detection settings (post-detection entity-kind allowlist).
    pub detection: Detection,
    /// Deduplication settings applied to combined detection results.
    pub deduplication: DeduplicationParams,
    /// Redaction settings applied after policy evaluation.
    pub redaction: Redaction,
    /// Validation settings for the post-redaction leak check.
    pub validation: Validation,
}
