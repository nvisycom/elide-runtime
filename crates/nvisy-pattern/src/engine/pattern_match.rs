//! [`RawMatch`]: internal output type from pattern scan phases.

use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RecognitionMethod};

use crate::patterns::ContextRule;

/// A single match produced by the pattern engine's internal scan phases.
///
/// Carries metadata needed for context adjustment and tracing before
/// conversion to [`Entity`].
#[derive(Debug, Clone)]
pub(crate) struct RawMatch {
    /// Name of the pattern that produced this match, or `None` for
    /// deny-list injected matches.
    pub pattern_name: Option<String>,
    /// Entity category of the match.
    pub category: EntityCategory,
    /// Entity kind of the match.
    pub entity_kind: EntityKind,
    /// Matched text.
    pub value: String,
    /// Byte offset of the match start in the input text.
    pub start: usize,
    /// Byte offset of the match end in the input text.
    pub end: usize,
    /// Confidence score assigned by the pattern definition.
    pub confidence: f64,
    /// Recognition methods that produced this match.
    pub recognition_methods: Vec<RecognitionMethod>,
    /// Optional context rule for co-occurrence confidence adjustment.
    pub context: Option<ContextRule>,
}

impl RawMatch {
    /// Apply context-based confidence adjustment.
    ///
    /// Searches the surrounding text (within `window` characters of the
    /// match boundaries) for any of the context rule's keywords. If at
    /// least one is found, boosts confidence by `boost`. If none are
    /// found, reduces confidence by `penalty`. Both are clamped to
    /// `[0.0, 1.0]`.
    pub fn apply_context_adjustment(&mut self, text: &str) {
        let rule = match &self.context {
            Some(r) => r,
            None => return,
        };

        let search_start = self.start.saturating_sub(rule.window);
        let search_end = (self.end + rule.window).min(text.len());
        let window_text = &text[search_start..search_end];

        let found = if rule.case_sensitive {
            rule.keywords
                .iter()
                .any(|kw| window_text.contains(kw.as_str()))
        } else {
            let lower = window_text.to_lowercase();
            rule.keywords
                .iter()
                .any(|kw| lower.contains(&kw.to_lowercase()))
        };

        if found {
            self.confidence = (self.confidence + rule.boost).clamp(0.0, 1.0);
        } else if rule.penalty > 0.0 {
            self.confidence = (self.confidence - rule.penalty).clamp(0.0, 1.0);
        }
    }

    /// Build an [`Entity`] from this match.
    ///
    /// The returned entity has no location or parent set: the caller
    /// should attach those from the span context.
    pub fn into_entity(self) -> Entity {
        debug_assert!(
            !self.recognition_methods.is_empty(),
            "RawMatch::into_entity requires at least one recognition method"
        );
        Entity::builder()
            .with_category(self.category)
            .with_entity_kind(self.entity_kind)
            .with_value(self.value)
            .with_recognition_methods(self.recognition_methods)
            .with_confidence(self.confidence)
            .build()
            .expect("required fields provided")
    }
}
