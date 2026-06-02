//! [`NerModelConfiguration`]: client-side NER tuning knobs.
//!
//! Mirrors Presidio's `NerModelConfiguration`. Applied inside the
//! adapter recognizers ([`NlpRecognizer`],
//! [`GlinerRecognizer`]) before entities
//! are emitted, so backends stay dumb and label normalization is
//! uniform across them.
//!
//! [`NlpRecognizer`]: super::NlpRecognizer
//! [`GlinerRecognizer`]: super::GlinerRecognizer

use std::collections::HashSet;

use nvisy_core::nlp::{AggregationStrategy, AlignmentMode};
use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::Confidence;

use super::label_map::LabelMap;

/// Per-recognizer NER policy.
///
/// Use [`NerModelConfiguration::default`] for the canonical
/// defaults (canonical label map, no ignored labels, score = 0.85,
/// no demotion) — these match what the current `inference-gliner`
/// service produces. Override individual fields when wiring
/// custom backends or applying low-confidence demotion policies.
#[derive(Debug, Clone)]
pub struct NerModelConfiguration {
    /// Translation from raw model labels to canonical
    /// [`EntityKind`] values. Defaults to
    /// [`LabelMap::canonical`].
    pub label_map: LabelMap,
    /// Raw labels the adapter drops without translation. Useful
    /// for filtering out labels the model emits but we don't care
    /// about (e.g. `O` from BIO tagging, `MISC` from generic NER
    /// schemas).
    pub labels_to_ignore: HashSet<String>,
    /// Fallback confidence when a backend emits a score outside
    /// `[0.0, 1.0]` (treated as a bug; clamped + this used as the
    /// safe default).
    pub default_score: Confidence,
    /// Entity kinds whose emitted confidence is multiplied by
    /// [`low_score_multiplier`] before
    /// being surfaced. Use for noisy-but-high-recall labels.
    ///
    /// [`low_score_multiplier`]: Self::low_score_multiplier
    pub low_score_kinds: HashSet<EntityKind>,
    /// Multiplier applied to [`low_score_kinds`].
    /// Must be in `[0.0, 1.0]`.
    ///
    /// [`low_score_kinds`]: Self::low_score_kinds
    pub low_score_multiplier: f64,
    /// Aggregation policy for backends that emit token-level
    /// predictions. Advisory for backends that aggregate
    /// server-side.
    pub aggregation: AggregationStrategy,
    /// Alignment policy for sub-word predictions. Same advisory
    /// status as [`aggregation`].
    ///
    /// [`aggregation`]: Self::aggregation
    pub alignment: AlignmentMode,
    /// Per-recognizer context-keyword list for the post-recognition
    /// [`ContextEnhancer`].
    /// Empty when the recognizer doesn't participate in boosting.
    /// Each emitted entity's source name keys the lookup, so the
    /// recognizer's [`name`] is used
    /// as the registration key.
    ///
    /// [`ContextEnhancer`]: nvisy_core::context::ContextEnhancer
    /// [`name`]: super::NlpRecognizer::name
    pub default_context: Vec<String>,
}

impl Default for NerModelConfiguration {
    fn default() -> Self {
        Self {
            label_map: LabelMap::canonical(),
            labels_to_ignore: HashSet::new(),
            default_score: Confidence::new(0.85).expect("0.85 in range"),
            low_score_kinds: HashSet::new(),
            low_score_multiplier: 0.4,
            aggregation: AggregationStrategy::Max,
            alignment: AlignmentMode::Expand,
            default_context: Vec::new(),
        }
    }
}

impl NerModelConfiguration {
    /// Construct a default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the [`LabelMap`].
    #[must_use]
    pub fn with_label_map(mut self, label_map: LabelMap) -> Self {
        self.label_map = label_map;
        self
    }

    /// Set the labels-to-ignore set.
    #[must_use]
    pub fn with_labels_to_ignore<I, S>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.labels_to_ignore = labels.into_iter().map(Into::into).collect();
        self
    }

    /// Override the default score.
    #[must_use]
    pub fn with_default_score(mut self, score: Confidence) -> Self {
        self.default_score = score;
        self
    }

    /// Set the low-score-demoted entity kinds.
    #[must_use]
    pub fn with_low_score_kinds<I>(mut self, kinds: I) -> Self
    where
        I: IntoIterator<Item = EntityKind>,
    {
        self.low_score_kinds = kinds.into_iter().collect();
        self
    }

    /// Set the multiplier applied to low-score kinds.
    #[must_use]
    pub fn with_low_score_multiplier(mut self, multiplier: f64) -> Self {
        self.low_score_multiplier = multiplier;
        self
    }

    /// Set the aggregation strategy.
    #[must_use]
    pub fn with_aggregation(mut self, aggregation: AggregationStrategy) -> Self {
        self.aggregation = aggregation;
        self
    }

    /// Set the alignment mode.
    #[must_use]
    pub fn with_alignment(mut self, alignment: AlignmentMode) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the per-recognizer context keywords. Becomes the entry
    /// in [`ContextRegistry`]
    /// keyed on the recognizer's name.
    ///
    /// [`ContextRegistry`]: nvisy_core::context::ContextRegistry
    #[must_use]
    pub fn with_default_context<I, S>(mut self, keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.default_context = keywords.into_iter().map(Into::into).collect();
        self
    }
}
