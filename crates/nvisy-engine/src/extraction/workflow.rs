//! Extraction node configuration.
//!
//! [`Extraction`] runs at **phase 1**, after ingestion. It converts
//! raw binary content into structured text that downstream detection
//! nodes can operate on. All applicable modalities always run — the
//! user controls *how* they run, not *whether* they run.
//!
//! The configuration is one per-modality struct per modality. The
//! top-level [`Extraction`] aggregate holds all four; the per-modality
//! [`Extract<M>`] impl picks its own slice via the [`Extract::Workflow`]
//! associated type.
//!
//! [`Extract<M>`]: super::Extract
//! [`Extract::Workflow`]: super::Extract::Workflow

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Text-modality workflow knobs. No tunables today; reserved for
/// future per-call settings (e.g. whitespace normalization).
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
pub struct TextWorkflow {}

/// Tabular-modality workflow knobs. No tunables today.
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
pub struct TabularWorkflow {}

/// Image-modality workflow knobs. No tunables today; reserved for
/// future OCR tuning (e.g. language hint, page subset).
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
pub struct ImageWorkflow {}

/// Audio-modality workflow knobs (speech-to-text).
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
pub struct AudialWorkflow {
    /// Segment the audio by speaker identity.
    ///
    /// Currently silently degrades to flat transcription — see #239.
    #[serde(default)]
    pub diarization: bool,
}

/// Unified extraction configuration.
///
/// Carries one per-modality workflow struct per modality. The
/// orchestrator dispatches `&plan.extraction.<modality>` to the
/// matching [`Extract<M>`] impl.
///
/// [`Extract<M>`]: super::Extract
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
pub struct Extraction {
    /// Text-modality workflow.
    #[serde(default)]
    pub text: TextWorkflow,
    /// Tabular-modality workflow.
    #[serde(default)]
    pub tabular: TabularWorkflow,
    /// Image-modality workflow (OCR).
    #[serde(default)]
    pub image: ImageWorkflow,
    /// Audial-modality workflow (STT + diarization).
    #[serde(default)]
    pub audial: AudialWorkflow,
}
