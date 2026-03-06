//! Structured output types for NER entity detection.

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::NerContext;

/// A list of NER entities returned by structured output.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NerEntities {
    /// Detected entities.
    pub entities: Vec<NerEntity>,
}

/// A single NER entity from structured LLM output.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct NerEntity {
    /// Stable identifier for the real-world entity this mention refers to.
    ///
    /// All mentions of the same person, organisation, etc. share the same
    /// `entity_id` (e.g. `"person_1"`). When known entities are provided
    /// as context, the LLM reuses their IDs for coreferent mentions.
    pub entity_id: String,
    /// Broad classification (may be absent for coreferent mentions like pronouns).
    pub category: Option<EntityCategory>,
    /// Specific entity type (may be absent for coreferent mentions like pronouns).
    pub entity_type: Option<EntityKind>,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0..=1.0).
    pub confidence: Option<f64>,
    /// A short snippet of surrounding text that uniquely locates this mention
    /// within the input. Used to compute byte offsets deterministically by
    /// finding `context` in the span, then `value` within the `context`.
    pub context: Option<String>,
    /// Brief description of the real-world entity (e.g. "CEO of Acme Corp,
    /// mentioned as the signatory"). Carried forward via [`KnownNerEntity`] so
    /// the LLM can disambiguate entities across chunks.
    pub description: Option<String>,
}

/// A previously identified entity carried as context between detection calls.
///
/// Lighter than [`NerEntity`] — holds only the information the LLM needs to
/// recognise and reuse an existing `entity_id`. Created via
/// [`NerContext::merge`].
#[derive(Debug, Clone, PartialEq)]
pub struct KnownNerEntity {
    /// Stable identifier (e.g. `"person_1"`).
    pub entity_id: String,
    /// Entity type, if known.
    pub entity_type: Option<EntityKind>,
    /// All surface forms seen so far (e.g. `["John Smith", "John", "Mr. Smith"]`).
    pub values: Vec<String>,
    /// Accumulated descriptions from successive detection calls.
    pub descriptions: Vec<String>,
}

/// Resolved byte offsets for an entity mention within its source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedOffsets {
    /// Start byte offset in the source text.
    pub start: usize,
    /// End byte offset (exclusive) in the source text.
    pub end: usize,
}

impl NerEntity {
    /// Resolve byte offsets of this entity's `value` within the text
    /// from the [`NerContext`] that produced it.
    ///
    /// When `context` is present, first locates the context snippet in
    /// the source text, then finds `value` within it. Falls back to
    /// searching for `value` directly in the source text when `context`
    /// is absent or not found.
    ///
    /// Returns `None` if the value cannot be located.
    pub fn resolve_offsets(&self, ctx: &NerContext<'_>) -> Option<ResolvedOffsets> {
        let text = ctx.text;

        if let Some(ref context) = self.context
            && let Some(ctx_start) = text.find(context.as_str())
            && let Some(val_offset) = context.find(&self.value)
        {
            let start = ctx_start + val_offset;
            return Some(ResolvedOffsets {
                start,
                end: start + self.value.len(),
            });
        }

        // Fallback: search for value directly in the source text.
        let start = text.find(&self.value)?;
        Some(ResolvedOffsets {
            start,
            end: start + self.value.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(value: &str, context: Option<&str>) -> NerEntity {
        NerEntity {
            entity_id: "test_1".into(),
            category: None,
            entity_type: None,
            value: value.into(),
            confidence: None,
            context: context.map(Into::into),
            description: None,
        }
    }

    #[test]
    fn resolve_with_context() {
        let text = "Alice met Bob. Later Alice called him.";
        let ctx = NerContext::new(text);
        let e = entity("Alice", Some("Later Alice called"));

        let offsets = e.resolve_offsets(&ctx).unwrap();
        assert_eq!(offsets.start, 21);
        assert_eq!(offsets.end, 26);
        assert_eq!(&text[offsets.start..offsets.end], "Alice");
    }

    #[test]
    fn resolve_without_context_finds_first() {
        let text = "Alice met Bob. Later Alice called him.";
        let ctx = NerContext::new(text);
        let e = entity("Alice", None);

        let offsets = e.resolve_offsets(&ctx).unwrap();
        assert_eq!(offsets.start, 0);
        assert_eq!(offsets.end, 5);
    }

    #[test]
    fn resolve_missing_value_returns_none() {
        let text = "No match here.";
        let ctx = NerContext::new(text);
        let e = entity("Charlie", Some("with Charlie"));

        assert!(e.resolve_offsets(&ctx).is_none());
    }

    #[test]
    fn resolve_context_not_found_falls_back() {
        let text = "Alice is here.";
        let ctx = NerContext::new(text);
        let e = entity("Alice", Some("stale context from another chunk"));

        let offsets = e.resolve_offsets(&ctx).unwrap();
        assert_eq!(offsets.start, 0);
        assert_eq!(offsets.end, 5);
    }

    #[test]
    fn resolve_disambiguates_duplicate_values() {
        let text = "He went home. She said he was tired.";
        let ctx = NerContext::new(text);

        let e1 = entity("he", Some("said he was"));
        let offsets = e1.resolve_offsets(&ctx).unwrap();
        assert_eq!(&text[offsets.start..offsets.end], "he");
        assert_eq!(offsets.start, 23);
    }
}
