//! Per-pass plan and per-modality knobs the detection pipeline
//! reads against each imported document.
//!
//! Three concerns share this file:
//!
//! - [`DetectionPlan`] — top-level per-request bundle the
//!   detection pipeline reads once per document.
//! - [`Extraction`] + per-modality plans ([`TextPlan`],
//!   [`TabularPlan`], [`ImagePlan`], [`AudioPlan`]) — the
//!   `Extraction` aggregate carries one plan struct per modality;
//!   the extraction phase dispatches `&plan.extraction.<modality>`
//!   to the matching backend.
//! - [`DeduplicationParams`] — re-export of the toolkit
//!   `LayerParams` type the deduplication phase consumes.

pub use nvisy_toolkit::deduplication::LayerParams as DeduplicationParams;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-request bundle of detection-side phase configs.
///
/// The detection pipeline reads this once per document and routes
/// each phase (extraction, then detection, then deduplication) to
/// the matching field. Detection itself has no plan node — its
/// per-request behaviour is driven by the policy-supplied label
/// catalog rather than a config object.
#[derive(Debug, Clone, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DetectionPlan {
    /// Extraction settings per modality.
    pub extraction: Extraction,
    /// Deduplication settings applied to combined detection results.
    pub deduplication: DeduplicationParams,
}

/// Unified extraction plan.
///
/// Carries one per-modality plan struct per modality. The
/// orchestrator dispatches `&plan.extraction.<modality>` to the
/// matching `ExtractDispatch<M>` impl.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Extraction {
    /// Text-modality plan.
    #[serde(default)]
    pub text: TextPlan,
    /// Tabular-modality plan.
    #[serde(default)]
    pub tabular: TabularPlan,
    /// Image-modality plan (OCR).
    #[serde(default)]
    pub image: ImagePlan,
    /// Audio-modality plan (STT + diarization).
    #[serde(default)]
    pub audio: AudioPlan,
}

/// Text-modality plan knobs. No tunables today; reserved for future
/// per-call settings (e.g. whitespace normalization).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TextPlan {}

/// Tabular-modality plan knobs. No tunables today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TabularPlan {}

/// Image-modality plan knobs. No tunables today; reserved for
/// future OCR tuning (e.g. language hint, page subset).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ImagePlan {}

/// Audio-modality plan knobs (speech-to-text).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AudioPlan {
    /// Segment the audio by speaker identity.
    ///
    /// Currently silently degrades to flat transcription — see #239.
    #[serde(default)]
    pub diarization: bool,
}
