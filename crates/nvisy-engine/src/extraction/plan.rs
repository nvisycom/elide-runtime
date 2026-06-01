//! Extraction node plan.
//!
//! [`Extraction`] runs at **phase 1**, after ingestion. It converts
//! raw binary content into structured text that downstream detection
//! nodes can operate on. All applicable modalities always run — the
//! user controls *how* they run, not *whether* they run.
//!
//! The plan is one per-modality struct per modality. The top-level
//! [`Extraction`] aggregate holds all four; the per-modality
//! [`ExtractDispatch<M>`] impl picks its own slice via the
//! [`ExtractDispatch::Plan`] associated type.
//!
//! [`ExtractDispatch<M>`]: super::ExtractDispatch
//! [`ExtractDispatch::Plan`]: super::ExtractDispatch::Plan

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Text-modality plan knobs. No tunables today; reserved for future
/// per-call settings (e.g. whitespace normalization).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub struct TextPlan {}

/// Tabular-modality plan knobs. No tunables today.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub struct TabularPlan {}

/// Image-modality plan knobs. No tunables today; reserved for future
/// OCR tuning (e.g. language hint, page subset).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub struct ImagePlan {}

/// Audio-modality plan knobs (speech-to-text).
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
pub struct AudioPlan {
    /// Segment the audio by speaker identity.
    ///
    /// Currently silently degrades to flat transcription — see #239.
    #[serde(default)]
    pub diarization: bool,
}

/// Unified extraction plan.
///
/// Carries one per-modality plan struct per modality. The
/// orchestrator dispatches `&plan.extraction.<modality>` to the
/// matching [`ExtractDispatch<M>`] impl.
///
/// [`ExtractDispatch<M>`]: super::ExtractDispatch
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
