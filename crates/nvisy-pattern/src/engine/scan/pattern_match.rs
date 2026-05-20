//! [`RawMatch`]: internal output type from pattern scan phases.
//!
//! Lives in the [`scan`] module because it is the shared exchange
//! type between [`phases`] and [`dedup`].
//!
//! [`scan`]: super
//! [`phases`]: super::phases
//! [`dedup`]: super::dedup

use nvisy_ontology::entity::{
    Entity, EntityCategory, EntityKind, Location, RecognitionMethod, TextLocation,
};
use nvisy_ontology::primitive::Confidence;
use smallvec::SmallVec;

use crate::patterns::ContextRule;

/// In-place storage for the common case of one or two recognition methods.
pub(crate) type RecognitionMethods = SmallVec<[RecognitionMethod; 2]>;

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
    pub recognition_methods: RecognitionMethods,
    /// Optional context rule for co-occurrence confidence adjustment.
    pub context: Option<ContextRule>,
}

impl RawMatch {
    /// Build an [`Entity`] from this match.
    ///
    /// The returned entity has no location or parent set: the caller
    /// should attach those from the span context.
    pub fn into_entity(self) -> Entity {
        debug_assert!(
            !self.recognition_methods.is_empty(),
            "RawMatch::into_entity requires at least one recognition method"
        );
        // RawMatch::confidence is bounded to [0,1] by the pattern
        // engine's context-aware enhancer (see ContextEnhancer in
        // scan/enhancer.rs). Clamp defensively to absorb any float
        // rounding before the Confidence constructor would reject it.
        let confidence =
            Confidence::new(self.confidence.clamp(0.0, 1.0)).expect("clamped value is in [0,1]");
        Entity::builder()
            .with_category(self.category)
            .with_entity_kind(self.entity_kind)
            .with_recognition_methods(self.recognition_methods.into_vec())
            .with_confidence(confidence)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_start_offset(self.start)
                    .with_end_offset(self.end)
                    .build()
                    .expect("required fields provided"),
            ))
            .build()
            .expect("required fields provided")
    }
}
