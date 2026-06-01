//! Per-entity score trail.
//!
//! [`Entity<M>::trail`] carries the chronological list of
//! [`TrailStep`]s explaining how the entity reached its final
//! confidence: which recognizer fired, which post-detection steps
//! adjusted the score, and the score before and after each step.
//!
//! This single trail replaces the prior parallel pair of
//! `recognition_methods` + `refinement_methods` and the standalone
//! "analysis explanation": every score-affecting event recorded
//! on the entity lives here. Consumers asking "who recognized this"
//! filter by [`TrailStepKind::Recognition`]; "what refinements ran"
//! by [`TrailStepKind::Refinement`] / [`TrailStepKind::Verification`]
//! / [`TrailStepKind::Fusion`] / [`TrailStepKind::Calibration`].
//!
//! `source` matches the recognizer's registration name in the
//! `DetectionEngine` for recognizer-produced steps; well-known
//! constants (`"dedup"`, `"calibration"`) cover post-recognition
//! steps. `reason` is free text so custom recognizers can describe
//! novel adjustments without extending an enum.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::provenance::{AnnotationProvenance, ModelProvenance, PatternProvenance};
use crate::primitive::Confidence;

/// One score-adjustment step in an [`Entity`]'s trail.
///
/// [`Entity`]: crate::entity::Entity
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrailStep {
    /// Identifier of the source that produced this step. For
    /// recognizer-produced steps this is the recognizer's
    /// registration name in the `DetectionEngine`
    /// (`"pattern"`, `"ner"`, `"llm"`, `"vlm"`, or a custom
    /// recognizer's name). For post-recognition steps it is a
    /// well-known constant (`"dedup"`, `"calibration"`).
    pub source: String,
    /// Which category of step this is — recognition, refinement,
    /// verification, fusion, or calibration.
    pub kind: TrailStepKind,
    /// Score before this step ran. `None` for the first step (no
    /// prior score to record).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Confidence>,
    /// Score after this step ran.
    pub adjusted: Confidence,
    /// Typed provenance details for this step. [`TrailProvenance::None`]
    /// when the step has no structured metadata to attach (e.g.
    /// dedup fusion, calibration adjustment).
    #[serde(default, skip_serializing_if = "TrailProvenance::is_none")]
    pub provenance: TrailProvenance,
    /// Free-text reason explaining what this step did and why.
    pub reason: String,
}

impl TrailStep {
    /// Construct a base [`Recognition`](TrailStepKind::Recognition)
    /// step — a recognizer firing for the first time. `original` is
    /// `None`.
    pub fn recognition(
        source: impl Into<String>,
        adjusted: Confidence,
        provenance: TrailProvenance,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            kind: TrailStepKind::Recognition,
            original: None,
            adjusted,
            provenance,
            reason: reason.into(),
        }
    }

    /// Construct a [`Refinement`](TrailStepKind::Refinement) step —
    /// a post-recognition score tweak (context boost or penalty,
    /// validator pass, etc.).
    pub fn refinement(
        source: impl Into<String>,
        original: Confidence,
        adjusted: Confidence,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            kind: TrailStepKind::Refinement,
            original: Some(original),
            adjusted,
            provenance: TrailProvenance::None,
            reason: reason.into(),
        }
    }

    /// Construct a [`Verification`](TrailStepKind::Verification)
    /// step — an LLM/VLM verify pass confirmed or rejected the
    /// detection.
    pub fn verification(
        source: impl Into<String>,
        original: Confidence,
        adjusted: Confidence,
        provenance: TrailProvenance,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            kind: TrailStepKind::Verification,
            original: Some(original),
            adjusted,
            provenance,
            reason: reason.into(),
        }
    }

    /// Construct a [`Fusion`](TrailStepKind::Fusion) step — dedup
    /// merged overlapping matches into this entity.
    pub fn fusion(original: Confidence, adjusted: Confidence, reason: impl Into<String>) -> Self {
        Self {
            source: "dedup".to_owned(),
            kind: TrailStepKind::Fusion,
            original: Some(original),
            adjusted,
            provenance: TrailProvenance::None,
            reason: reason.into(),
        }
    }

    /// Construct a [`Calibration`](TrailStepKind::Calibration) step
    /// — the per-method calibration multiplier was applied.
    pub fn calibration(
        original: Confidence,
        adjusted: Confidence,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source: "calibration".to_owned(),
            kind: TrailStepKind::Calibration,
            original: Some(original),
            adjusted,
            provenance: TrailProvenance::None,
            reason: reason.into(),
        }
    }
}

/// Discriminant for [`TrailStep`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum TrailStepKind {
    /// Base step: a recognizer fired and produced this entity for
    /// the first time.
    Recognition,
    /// Post-recognition tweak (validator pass, context-rule boost
    /// or penalty, contextual adjustment).
    Refinement,
    /// LLM/VLM verify pass confirmed or rejected the detection.
    Verification,
    /// Deduplication merged overlapping matches into this entity.
    Fusion,
    /// Per-method calibration multiplier was applied.
    Calibration,
}

/// Typed provenance metadata attached to a [`TrailStep`].
///
/// Carries the structured details for built-in step kinds (pattern
/// name, model name, annotation name). Custom recognizers register
/// under their own name and rely on the step's `reason` for
/// human-readable detail, leaving this field as
/// [`TrailProvenance::None`].
#[derive(Debug, Clone, PartialEq, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TrailProvenance {
    /// Pattern-based detection — regex, dictionary, or deny-list.
    Pattern(PatternProvenance),
    /// Model-based detection — NER, LLM, or VLM. The
    /// [`ModelProvenance`] carries name + kind + optional version.
    Model(ModelProvenance),
    /// Annotation-derived detection — a hint or assertion supplied
    /// alongside the uploaded file materialised into an entity.
    Annotation(AnnotationProvenance),
    /// No structured provenance for this step. Used by custom
    /// recognizers and by post-recognition steps (dedup, calibration)
    /// whose details fit in the `reason` text.
    #[default]
    None,
}

impl TrailProvenance {
    /// True when this provenance carries no structured data.
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}
