//! Build [`Entity<Text>`] values from localized [`NerCandidate`]s.
//!
//! Used by the [`detect`] pass to lift LLM-produced candidates into
//! entities. The recognition trail step is computed per-candidate
//! via the caller-supplied closure: candidates carrying a `hint_id`
//! get an [`Annotation`] provenance
//! with the hint's name; fresh discoveries get a
//! [`Model`] provenance carrying the LLM's
//! identity.
//!
//! Candidates missing an `entity_type` are dropped (we don't invent
//! a kind); candidates whose `confidence` falls outside `[0, 1]`
//! after clamping are also dropped.
//!
//! [`detect`]: super::NerAgent::detect
//! [`Annotation`]: TrailProvenance::Annotation
//! [`Model`]: TrailProvenance::Model

use nvisy_ontology::entity::{Entity, TrailStep};
use nvisy_ontology::modality::Text;
use nvisy_ontology::primitive::Confidence;

use super::localize::LocalizedCandidate;

/// Default confidence assigned to a candidate when the LLM didn't
/// score it.
const DEFAULT_CONFIDENCE: f64 = 0.5;

const TARGET: &str = "nvisy_agent::agent::ner::build";

/// Build entities from a localized candidate set, computing the
/// recognition trail step per candidate via `step_for`. Logs (at
/// debug) the count of candidates dropped for missing kind or bad
/// confidence.
pub(crate) fn build_entities<F>(
    localized: Vec<LocalizedCandidate>,
    mut step_for: F,
) -> Vec<Entity<Text>>
where
    F: FnMut(&LocalizedCandidate, Confidence) -> TrailStep,
{
    let mut out = Vec::with_capacity(localized.len());
    let mut dropped_missing_kind = 0usize;
    let mut dropped_bad_confidence = 0usize;

    for l in localized {
        let Some(entity_kind) = l.candidate.entity_type else {
            dropped_missing_kind += 1;
            continue;
        };
        let raw = l.candidate.confidence.unwrap_or(DEFAULT_CONFIDENCE);
        let Some(confidence) = Confidence::new(raw.clamp(0.0, 1.0)) else {
            dropped_bad_confidence += 1;
            continue;
        };
        let loc = Text::new(l.start_offset, l.end_offset);
        let step = step_for(&l, confidence);

        let mut b = Entity::builder()
            .with_entity_kind(entity_kind)
            .with_trail(vec![step])
            .with_confidence(confidence)
            .with_location(loc);
        if let Some(id) = l.candidate.entity_id {
            b = b.with_entity_id(id);
        }
        out.push(b.build().expect("required fields provided"));
    }

    if dropped_missing_kind > 0 || dropped_bad_confidence > 0 {
        tracing::debug!(
            target: TARGET,
            dropped_missing_kind,
            dropped_bad_confidence,
            "dropped candidates during entity construction"
        );
    }
    out
}
