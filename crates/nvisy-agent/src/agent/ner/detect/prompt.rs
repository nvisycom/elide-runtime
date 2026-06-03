//! Prompt construction for the unified NER detect pass.
//!
//! The detect pass does two things in one LLM call:
//!
//! 1. Open-ended discovery: find every sensitive entity in the
//!    source text. Each candidate carries its own confidence;
//!    threshold filtering happens later in the engine's
//!    deduplication step, not in the prompt.
//! 2. Per-hint adjudication: for each uploader-supplied
//!    [`Hint<Text>`] on [`RecognizerInput::hints`], either emit a
//!    candidate carrying `hint_id = Some(<that hint's index>)`
//!    (confirming or relocating it) or omit any reference to it
//!    (implicit rejection).
//!
//! Both kinds of output share the [`NerCandidate`] shape; only the
//! optional `hint_id` distinguishes them. Provenance stamping
//! (`Annotation` for hint responses, `Model` for discoveries) is
//! handled downstream of this prompt by the agent.
//!
//! [`Hint<Text>`]: nvisy_core::Hint
//! [`RecognizerInput::hints`]: nvisy_core::RecognizerInput::hints
//! [`NerCandidate`]: super::NerCandidate

use nvisy_core::Hint;
use nvisy_core::modality::Text;

/// Builds user prompts for the unified detect pass from a per-call
/// text + hints + labels triple.
pub(crate) struct NerPromptBuilder<'a> {
    text: &'a str,
    hints: &'a [Hint<Text>],
    labels: &'a [String],
}

impl<'a> NerPromptBuilder<'a> {
    /// Create a prompt builder from the per-call source text, the
    /// uploader-supplied hints, and the document labels.
    pub fn new(text: &'a str, hints: &'a [Hint<Text>], labels: &'a [String]) -> Self {
        Self {
            text,
            hints,
            labels,
        }
    }

    /// Build the user prompt.
    pub fn build(&self) -> String {
        let mut prompt = String::new();
        self.render_instruction(&mut prompt);
        self.render_source(&mut prompt);
        self.render_labels(&mut prompt);
        self.render_hints(&mut prompt);
        prompt
    }

    fn render_instruction(&self, prompt: &mut String) {
        prompt.push_str(
            "Detect every sensitive entity in the following text. \
             Return a JSON object with an \"entities\" key whose value is an array of \
             candidates. Each candidate has keys: entity_id, entity_type, \
             value, confidence, context, description, hint_id.",
        );
    }

    fn render_source(&self, prompt: &mut String) {
        prompt.push_str("\n\n---\n");
        prompt.push_str(self.text);
        prompt.push_str("\n---");
    }

    fn render_labels(&self, prompt: &mut String) {
        if self.labels.is_empty() {
            return;
        }
        let labels = self.labels.join(", ");
        prompt.push_str(&format!(
            "\n\nDocument context labels (adjust sensitivity to \
             domain-specific terms accordingly): {labels}."
        ));
    }

    /// Render the per-hint section. Each hint gets an indexed line
    /// with its claimed metadata and a snippet so the LLM has
    /// enough context to confirm, relocate, or reject. Empty when
    /// there are no hints.
    fn render_hints(&self, prompt: &mut String) {
        if self.hints.is_empty() {
            return;
        }
        prompt.push_str(
            "\n\nThe uploader marked these regions as likely sensitive. For each \
             hint, emit a candidate with hint_id set to the hint's index if you \
             confirm or relocate it (use the candidate's value and context fields \
             to point at the corrected location), or omit any reference to that \
             hint_id to reject it. Hints:",
        );
        for (i, h) in self.hints.iter().enumerate() {
            let value = value_at(self.text, h.location.start, h.location.end);
            let snippet = snippet_around(self.text, h.location.start, h.location.end);
            let name = h.name.as_deref().unwrap_or("");
            let kind = h
                .entity_kind
                .map(|k| k.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            prompt.push_str(&format!(
                "\n[hint {i}] name=\"{name}\", kind={kind}, \
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
candidate objects. Each candidate has keys: entity_id (optional), \
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
    use nvisy_core::entity::EntityKind;
    use nvisy_core::modality::Text;

    use super::*;

    #[test]
    fn renders_source_text() {
        let prompt = NerPromptBuilder::new("Hello world", &[], &[]).build();
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("Detect every sensitive entity"));
    }

    #[test]
    fn renders_labels_when_present() {
        let labels = vec!["medical".to_string(), "internal".to_string()];
        let prompt = NerPromptBuilder::new("some text", &[], &labels).build();
        assert!(prompt.contains("Document context labels"));
        assert!(prompt.contains("medical"));
        assert!(prompt.contains("internal"));
    }

    #[test]
    fn omits_label_section_when_empty() {
        let prompt = NerPromptBuilder::new("plain", &[], &[]).build();
        assert!(!prompt.contains("Document context labels"));
    }

    #[test]
    fn renders_hints_with_index_value_and_snippet() {
        let text = "Hello Alice, your invoice 12345 is ready.";
        let alice_start = text.find("Alice").unwrap();
        let hints = vec![
            Hint::new(Text::new(alice_start, alice_start + 5))
                .with_name("customer")
                .with_entity_kind(EntityKind::PersonName),
        ];
        let prompt = NerPromptBuilder::new(text, &hints, &[]).build();
        assert!(prompt.contains("[hint 0]"));
        assert!(prompt.contains("name=\"customer\""));
        assert!(prompt.contains("kind=person_name"));
        assert!(prompt.contains("value=\"Alice\""));
        assert!(prompt.contains("snippet:"));
        assert!(prompt.contains("hint_id"));
    }

    #[test]
    fn omits_hint_section_when_empty() {
        let prompt = NerPromptBuilder::new("plain text", &[], &[]).build();
        assert!(!prompt.contains("[hint"));
        assert!(!prompt.contains("uploader marked these regions"));
    }
}
