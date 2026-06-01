//! Whole-audit verifier prompt construction.
//!
//! Lists each input [`Entity<Text>`] by index along with a snippet
//! of surrounding source text so the LLM can re-judge it. The
//! verdict references entities by the same index.

use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

/// Window of source text emitted around each entity (in bytes on
/// either side of its localized range).
const SNIPPET_HALF_WIDTH: usize = 80;

/// Builds whole-audit verifier prompts.
pub(super) struct NerVerifyPromptBuilder<'a> {
    text: &'a str,
    entities: &'a [Entity<Text>],
}

impl<'a> NerVerifyPromptBuilder<'a> {
    pub(super) fn new(text: &'a str, entities: &'a [Entity<Text>]) -> Self {
        Self { text, entities }
    }

    pub(super) fn build(&self) -> String {
        let mut prompt = String::from(
            "Verify each proposed entity below against its snippet from \
             the source text. Confirm by omission, or return a verdict to \
             reject false positives or adjust (kind / confidence) on a \
             per-entity basis.\n\n\
             Proposed entities:\n",
        );

        for (i, e) in self.entities.iter().enumerate() {
            let start = e.location.start;
            let end = e.location.end;
            let snippet_start =
                floor_char_boundary(self.text, start.saturating_sub(SNIPPET_HALF_WIDTH));
            let snippet_end =
                ceil_char_boundary(self.text, (end + SNIPPET_HALF_WIDTH).min(self.text.len()));
            let snippet = &self.text[snippet_start..snippet_end];
            let value = if start < end && end <= self.text.len() {
                &self.text[start..end]
            } else {
                ""
            };

            prompt.push_str(&format!(
                "[{i}] kind={kind}, value=\"{val}\", confidence={conf:.2}\n  snippet: \"{snip}\"\n",
                kind = e.entity_kind,
                val = value,
                conf = e.confidence.get(),
                snip = snippet,
            ));
        }

        prompt
    }
}

/// Default system prompt for the whole-audit verifier.
pub(super) const NER_VERIFIER_SYSTEM_PROMPT: &str = "\
You verify proposed sensitive-data recognitions against the source \
text. You receive a list of entities (each with an id, kind, value, \
confidence, and a short snippet of surrounding text) that were \
identified by upstream recognizers.\n\
\n\
Your task is to re-read the snippets and return only entities that \
need changes. Return a JSON object with an \"entities\" key \
containing an array of changed entries:\n\
\n\
- **rejected**: the entity is a false positive at this location.\n\
- **corrected**: the entity exists at this location but you want \
  to adjust its kind or confidence. Provide whichever fields you \
  wish to update. The entity's location and value are fixed by the \
  original recognizer and cannot be changed here.\n\
\n\
Entities that are correct as-is should NOT appear in your response. \
If every entity is correct, return {\"entities\": []}.\n\
\n\
Each entry must have: id (the index from the prompt), status \
(\"corrected\" or \"rejected\"), confidence (0.0-1.0). For corrected \
entities, optionally include entity_type. The value and bbox \
fields, if present, are ignored.";

fn floor_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn ceil_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityKind, ModelProvenance, TrailProvenance, TrailStep};
    use nvisy_ontology::primitive::Confidence;

    use super::*;

    fn entity(start: usize, end: usize, kind: EntityKind) -> Entity<Text> {
        let confidence = Confidence::new(0.5).unwrap();
        let step = TrailStep::recognition(
            "llm-ner",
            confidence,
            TrailProvenance::Model(ModelProvenance::new("test")),
            "",
        );
        Entity::builder()
            .with_entity_kind(kind)
            .with_trail(vec![step])
            .with_confidence(confidence)
            .with_location(Text::new(start, end))
            .build()
            .unwrap()
    }

    #[test]
    fn includes_index_value_and_snippet() {
        let text = "Hello Alice, your invoice 12345 is ready.";
        let entities = vec![entity(6, 11, EntityKind::PersonName)];
        let prompt = NerVerifyPromptBuilder::new(text, &entities).build();
        assert!(prompt.contains("[0]"));
        assert!(prompt.contains("value=\"Alice\""));
        assert!(prompt.contains("kind=person_name"));
        assert!(prompt.contains("snippet:"));
    }

    #[test]
    fn snippet_respects_utf8_boundaries() {
        // Multi-byte char near the snippet edge — must not slice
        // mid-codepoint.
        let text = "café — Alice — café";
        let alice_start = text.find("Alice").unwrap();
        let entities = vec![entity(alice_start, alice_start + 5, EntityKind::PersonName)];
        // Builder should not panic; UTF-8 boundary walks ensure this.
        let _ = NerVerifyPromptBuilder::new(text, &entities).build();
    }
}
