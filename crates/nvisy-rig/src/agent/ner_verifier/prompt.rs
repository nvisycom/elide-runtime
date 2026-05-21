//! NER-verifier-specific prompt construction.
//!
//! [`NerVerifierPromptBuilder`] constructs the user prompt that
//! lists each localized candidate plus a snippet of surrounding
//! text for the LLM to vote keep/correct/reject against.

use super::DEFAULT_CONFIDENCE;
use super::localize::LocalizedCandidate;

/// Window of source text emitted around each candidate (in bytes
/// on either side of the candidate's localized range).
const SNIPPET_HALF_WIDTH: usize = 80;

/// Builds user prompts for NER candidate verification.
pub(super) struct NerVerifierPromptBuilder<'a> {
    text: &'a str,
    localized: &'a [LocalizedCandidate],
}

impl<'a> NerVerifierPromptBuilder<'a> {
    /// Create a prompt builder for the given source text and
    /// localized candidates.
    pub(super) fn new(text: &'a str, localized: &'a [LocalizedCandidate]) -> Self {
        Self { text, localized }
    }

    /// Build the user prompt.
    pub(super) fn build(&self) -> String {
        let mut prompt = String::from(
            "Verify each proposed entity below against its snippet from \
             the source text. Vote keep/correct/reject per entry.\n\n\
             Proposed entities:\n",
        );

        for (i, l) in self.localized.iter().enumerate() {
            let snippet_start = l.start_offset.saturating_sub(SNIPPET_HALF_WIDTH);
            let snippet_end = (l.end_offset + SNIPPET_HALF_WIDTH).min(self.text.len());
            // Walk to nearest char boundaries.
            let snippet_start = next_char_boundary_lo(self.text, snippet_start);
            let snippet_end = next_char_boundary_hi(self.text, snippet_end);
            let snippet = &self.text[snippet_start..snippet_end];

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
            let entity_id = l.candidate.entity_id.as_deref().unwrap_or("none");

            prompt.push_str(&format!(
                "[{i}] entity_id={entity_id}, category={category}, type={kind}, \
                 value=\"{}\", confidence={confidence:.2}\n  snippet: \"{}\"\n",
                l.candidate.value, snippet,
            ));
        }

        prompt
    }
}

/// Default system prompt for the NER verifier.
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
