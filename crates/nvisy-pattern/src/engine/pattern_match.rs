//! [`RawMatch`]: output type from pattern scanning.

use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RecognitionMethod};

use crate::patterns::ContextRule;

/// A single match produced by the pattern engine's internal scan phases.
#[derive(Debug, Clone)]
pub(crate) struct RawMatch {
    /// Name of the pattern that produced this match, or `None` for
    /// deny-list injected matches. Read in tests for match identification.
    #[cfg_attr(not(test), allow(dead_code))]
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
    /// Recognition methods that produced this match, ordered by
    /// application time (e.g. `[Regex, Checksum]` when a regex
    /// match was confirmed by a validator).
    pub recognition_methods: Vec<RecognitionMethod>,
    /// Optional context rule for span-level co-occurrence scoring.
    /// Read in tests for context rule validation.
    #[cfg_attr(not(test), allow(dead_code))]
    pub context: Option<ContextRule>,
}

impl RawMatch {
    /// Build an [`Entity`] from this match.
    ///
    /// The returned entity has no location or parent set: the caller
    /// should attach those from the span context via
    /// [`Entity::with_location`] and [`Entity::with_parent`].
    ///
    /// # Panics
    ///
    /// Panics if `recognition_methods` is empty. All engine-produced
    /// matches always carry at least one method.
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
