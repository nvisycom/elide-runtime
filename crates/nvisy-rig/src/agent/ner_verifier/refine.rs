//! LLM refinement pass for localized NER candidates.
//!
//! Builds a verification prompt enumerating each localized
//! candidate plus a snippet of surrounding text, asks the LLM to
//! vote keep/correct/reject per entry, and folds the verdicts back
//! into the localized list.

use nvisy_core::Result;

use super::DEFAULT_CONFIDENCE;
use super::localize::LocalizedCandidate;
use crate::agent::base::{BaseAgent, VerificationOutput, VerificationStatus};

/// System prompt for the NER verifier.
pub(super) const NER_VERIFIER_SYSTEM_PROMPT: &str = "\
You verify proposed named-entity recognitions against the source \
text. You receive a list of entities (each with an id, optional \
category and type, value, confidence, and a short snippet of \
surrounding text) that were identified by an upstream NER system.\n\
\n\
Your task is to re-read the snippets in light of the source text \
and return only entities that need changes. Return a JSON object \
with an \"entities\" key containing an array of changed entries:\n\
\n\
- **corrected**: the entity exists at the snippet's location but \
  has a wrong value, type, or category. Include the corrected \
  fields (category, entity_type, value).\n\
- **rejected**: the entity is a false positive at this location.\n\
\n\
Entities that are correct should NOT appear in your response. If \
all entities are correct, return {\"entities\": []}.\n\
\n\
Each entry must have: id (matching the proposed entity's id), \
status (\"corrected\" or \"rejected\"), confidence (0.0-1.0). For \
corrected entities, also include whichever fields changed: \
category, entity_type, value.";

/// Window of source text emitted around each candidate (in bytes
/// either side of the candidate's localized range).
const SNIPPET_HALF_WIDTH: usize = 80;

/// Run the refinement pass over a slice of localized candidates.
///
/// Returns the surviving (kept or corrected) candidates. Rejected
/// entries are dropped; corrected entries have their fields
/// updated in place.
pub(super) async fn refine_localized(
    agent: &BaseAgent,
    text: &str,
    localized: Vec<LocalizedCandidate>,
) -> Result<Vec<LocalizedCandidate>> {
    if localized.is_empty() {
        return Ok(localized);
    }

    let prompt = build_prompt(text, &localized);
    let output: VerificationOutput = agent
        .prompt_structured_raw(&prompt)
        .await
        .map_err(crate::error::convert)?;

    Ok(apply_verdicts(localized, output))
}

fn build_prompt(text: &str, localized: &[LocalizedCandidate]) -> String {
    let mut prompt = String::from(
        "Verify each proposed entity below against its snippet from \
         the source text. Vote keep/correct/reject per entry.\n\n\
         Proposed entities:\n",
    );

    for (i, l) in localized.iter().enumerate() {
        let snippet_start = l.start_offset.saturating_sub(SNIPPET_HALF_WIDTH);
        let snippet_end = (l.end_offset + SNIPPET_HALF_WIDTH).min(text.len());
        // Walk to nearest char boundaries.
        let snippet_start = next_char_boundary_lo(text, snippet_start);
        let snippet_end = next_char_boundary_hi(text, snippet_end);
        let snippet = &text[snippet_start..snippet_end];

        let kind = l
            .candidate
            .entity_type
            .map(|k| k.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let category = l
            .candidate
            .category
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let confidence = l.candidate.confidence.unwrap_or(DEFAULT_CONFIDENCE);

        prompt.push_str(&format!(
            "[{i}] entity_id={}, category={category}, type={kind}, \
             value=\"{}\", confidence={confidence:.2}\n  snippet: \"{}\"\n",
            l.candidate.entity_id, l.candidate.value, snippet,
        ));
    }

    prompt
}

fn apply_verdicts(
    mut localized: Vec<LocalizedCandidate>,
    output: VerificationOutput,
) -> Vec<LocalizedCandidate> {
    use std::collections::HashMap;

    let verdicts: HashMap<usize, _> = output.entities.into_iter().map(|v| (v.id, v)).collect();

    let mut out = Vec::with_capacity(localized.len());
    for (i, mut l) in localized.drain(..).enumerate() {
        match verdicts.get(&i) {
            None => out.push(l), // implicit confirm
            Some(v) => match v.status {
                VerificationStatus::Rejected => {} // drop
                VerificationStatus::Corrected => {
                    if let Some(ref new_value) = v.value {
                        l.candidate.value = new_value.clone();
                    }
                    if v.category.is_some() {
                        l.candidate.category = v.category;
                    }
                    if v.entity_type.is_some() {
                        l.candidate.entity_type = v.entity_type;
                    }
                    // Update confidence to the verifier's (more
                    // recent) judgement.
                    l.candidate.confidence = Some(v.confidence);
                    out.push(l);
                }
            },
        }
    }
    out
}

fn next_char_boundary_lo(s: &str, mut pos: usize) -> usize {
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn next_char_boundary_hi(s: &str, mut pos: usize) -> usize {
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}
