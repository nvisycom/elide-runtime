//! [`Plan`]: the per-request bundle of per-phase configs that the
//! pipeline executes against each imported document.
//!
//! Split out from [`EngineInput`] so the request envelope (identity,
//! imports, exports, contexts, dry-run flag) stays cleanly separated
//! from the pipeline behaviour knobs each phase reads.
//!
//! [`EngineInput`]: super::EngineInput

use crate::deduplication::DeduplicationParams;
use crate::detection::Detection;
use crate::extraction::Extraction;
use crate::redaction::Redaction;
use crate::validation::Validation;

/// Per-request bundle of per-phase configs.
///
/// The pipeline reads this once per document and routes each phase
/// to the matching field. Order of execution is fixed
/// (extraction → detection → dedup → redaction → validation); fields
/// here are configuration only, not sequencing.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Extraction settings per modality.
    pub extraction: Extraction,
    /// Detection settings (which recognizer kinds + per-call hints).
    pub detection: Detection,
    /// Deduplication settings applied to combined detection results.
    pub deduplication: DeduplicationParams,
    /// Redaction settings applied after policy evaluation.
    pub redaction: Redaction,
    /// Validation settings for the post-redaction leak check.
    pub validation: Validation,
}
