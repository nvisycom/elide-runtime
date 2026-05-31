//! Prompt construction for the unified NER detect pass.
//!
//! The detect pass does two things in one LLM call:
//!
//! 1. Open-ended discovery: find every sensitive entity in the
//!    source text matching the configured `entity_kinds` (or all
//!    kinds when empty). Each candidate carries its own confidence;
//!    threshold filtering happens later in the engine's
//!    deduplication step, not in the prompt.
//! 2. Per-hint adjudication: for each user-supplied [`NerHint`] in
//!    [`LlmNerContext::hints`], either emit a candidate carrying
//!    `hint_id = Some(<that hint's index>)` (confirming or
//!    relocating it) or omit any reference to it (implicit
//!    rejection).
//!
//! Both kinds of output share the [`NerCandidate`] shape; only the
//! optional `hint_id` distinguishes them. Provenance stamping
//! (`Annotation` for hint responses, `LlmNer` for discoveries) is
//! handled downstream of this prompt by the agent.
//!
//! [`NerHint`]: crate::agent::NerHint
//! [`LlmNerContext::hints`]: crate::agent::LlmNerContext::hints
//! [`NerCandidate`]: super::NerCandidate

use crate::agent::{ALL_TYPES_HINT, LlmNerContext};

/// Builds user prompts for the unified detect pass.
pub(crate) struct NerPromptBuilder<'a> {
    config: &'a LlmNerContext,
}

impl<'a> NerPromptBuilder<'a> {
    /// Create a prompt builder from a [`LlmNerContext`].
    pub fn new(config: &'a LlmNerContext) -> Self {
        Self { config }
    }

    /// Build the user prompt for the given text.
    pub fn build(&self, text: &str) -> String {
        let mut prompt = String::new();
        self.render_instruction(&mut prompt);
        self.render_source(&mut prompt, text);
        self.render_labels(&mut prompt);
        self.render_hints(&mut prompt, text);
        prompt
    }

    fn render_instruction(&self, prompt: &mut String) {
        let types = if self.config.entity_kinds.is_empty() {
            ALL_TYPES_HINT.to_string()
        } else {
            self.config
                .entity_kinds
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        prompt.push_str(&format!(
            "Detect entities of types [{types}] in the following text. \
             Return a JSON object with an \"entities\" key whose value is an array of \
             candidates. Each candidate has keys: entity_id, category, entity_type, \
             value, confidence, context, description, hint_id."
        ));
    }

    fn render_source(&self, prompt: &mut String, text: &str) {
        prompt.push_str("\n\n---\n");
        prompt.push_str(text);
        prompt.push_str("\n---");
    }

    fn render_labels(&self, prompt: &mut String) {
        if self.config.labels.is_empty() {
            return;
        }
        let labels = self.config.labels.join(", ");
        prompt.push_str(&format!(
            "\n\nDocument context labels (adjust sensitivity to \
             domain-specific terms accordingly): {labels}."
        ));
    }

    /// Render the per-hint section. Each hint gets an indexed line
    /// with its claimed metadata and a snippet so the LLM has
    /// enough context to confirm, relocate, or reject. Empty when
    /// there are no hints.
    fn render_hints(&self, prompt: &mut String, text: &str) {
        if self.config.hints.is_empty() {
            return;
        }
        prompt.push_str(
            "\n\nThe uploader marked these regions as likely sensitive. For each \
             hint, emit a candidate with hint_id set to the hint's index if you \
             confirm or relocate it (use the candidate's value and context fields \
             to point at the corrected location), or omit any reference to that \
             hint_id to reject it. Hints:",
        );
        for (i, h) in self.config.hints.iter().enumerate() {
            let value = value_at(text, h.start, h.end);
            let snippet = snippet_around(text, h.start, h.end);
            let name = h.name.as_deref().unwrap_or("");
            let category = h
                .category
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let kind = h
                .entity_kind
                .map(|k| k.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            prompt.push_str(&format!(
                "\n[hint {i}] name=\"{name}\", category={category}, kind={kind}, \
                 value=\"{value}\"\n  snippet: \"{snippet}\""
            ));
        }
    }
}

/// Default system prompt for the unified NER detect pass.
pub(super) const NER_SYSTEM_PROMPT: &str = "\
You are a precise named-entity recognition system. \
Identify personally identifiable information (PII), protected health information (PHI), \
financial data, and credentials in the provided text.\n\
\n\
Return results as a JSON object with an \"entities\" key containing an array of \
candidate objects. Each candidate has keys: entity_id (optional), category (optional), \
entity_type (optional), value, confidence (optional), context (optional), \
description (optional), hint_id (optional).\n\
\n\
Set hint_id to a hint's index when the candidate is your response to that hint — \
either confirming it (use the same value+context that locates the hint) or relocating \
it (use the corrected value+context). To implicitly reject a hint, omit any candidate \
referring to it. Fresh discoveries (not tied to any hint) omit hint_id.\n\
\n\
Assign a stable entity_id (e.g. \"person_1\", \"org_1\") to each unique real-world \
entity; coreferent mentions of the same entity share entity_id.\n\
\n\
The \"context\" field is a short surrounding snippet of source text that uniquely \
locates this mention within the input. Include enough words before and after the value \
so the context string appears exactly once in the input text. This is critical when \
the same value (e.g. \"he\") appears multiple times.\n\
\n\
The \"description\" field is a brief description of the real-world entity (e.g. \"CEO \
of Acme Corp\", \"patient's home address\"). Provide it for the first mention of each \
entity or when additional context becomes available.\n\
\n\
If no entities are found, return {\"entities\": []}.";

/// Snippet window (in bytes) emitted on each side of a hint's
/// range so the LLM has surrounding context for judgement.
const SNIPPET_HALF_WIDTH: usize = 80;

fn snippet_around(text: &str, start: usize, end: usize) -> &str {
    let lo = floor_char_boundary(text, start.saturating_sub(SNIPPET_HALF_WIDTH));
    let hi = ceil_char_boundary(text, (end + SNIPPET_HALF_WIDTH).min(text.len()));
    &text[lo..hi]
}

fn value_at(text: &str, start: usize, end: usize) -> &str {
    if start < end
        && end <= text.len()
        && text.is_char_boundary(start)
        && text.is_char_boundary(end)
    {
        &text[start..end]
    } else {
        ""
    }
}

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
    use nvisy_ontology::entity::EntityKind;

    use super::*;
    use crate::agent::NerHint;

    #[test]
    fn renders_entity_kinds() {
        let config = LlmNerContext {
            entity_kinds: vec![EntityKind::PersonName, EntityKind::GovernmentId],
            ..Default::default()
        };
        let prompt = NerPromptBuilder::new(&config).build("Hello world");
        assert!(prompt.contains("person_name, government_id"));
        assert!(prompt.contains("Hello world"));
    }

    #[test]
    fn renders_all_kinds_when_empty() {
        let config = LlmNerContext::default();
        let prompt = NerPromptBuilder::new(&config).build("test");
        assert!(prompt.contains("all entity types"));
    }

    #[test]
    fn renders_labels_when_present() {
        let config = LlmNerContext {
            labels: vec!["medical".into(), "internal".into()],
            ..Default::default()
        };
        let prompt = NerPromptBuilder::new(&config).build("some text");
        assert!(prompt.contains("Document context labels"));
        assert!(prompt.contains("medical"));
        assert!(prompt.contains("internal"));
    }

    #[test]
    fn omits_label_section_when_empty() {
        let config = LlmNerContext::default();
        let prompt = NerPromptBuilder::new(&config).build("plain");
        assert!(!prompt.contains("Document context labels"));
    }

    #[test]
    fn renders_hints_with_index_value_and_snippet() {
        let text = "Hello Alice, your invoice 12345 is ready.";
        let alice_start = text.find("Alice").unwrap();
        let config = LlmNerContext {
            hints: vec![NerHint {
                name: Some("customer".into()),
                category: None,
                entity_kind: Some(EntityKind::PersonName),
                start: alice_start,
                end: alice_start + 5,
            }],
            ..Default::default()
        };
        let prompt = NerPromptBuilder::new(&config).build(text);
        assert!(prompt.contains("[hint 0]"));
        assert!(prompt.contains("name=\"customer\""));
        assert!(prompt.contains("kind=person_name"));
        assert!(prompt.contains("value=\"Alice\""));
        assert!(prompt.contains("snippet:"));
        assert!(prompt.contains("hint_id"));
    }

    #[test]
    fn omits_hint_section_when_empty() {
        let config = LlmNerContext::default();
        let prompt = NerPromptBuilder::new(&config).build("plain text");
        assert!(!prompt.contains("[hint"));
        assert!(!prompt.contains("uploader marked these regions"));
    }
}
