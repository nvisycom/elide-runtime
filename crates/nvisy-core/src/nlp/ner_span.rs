//! [`RawNerSpan`]: pre-normalization NER prediction.
//!
//! Emitted by an `NlpEngine` that runs a NER model (or proxies one
//! over a remote service). The label is the *raw* string the model
//! produced — `PER`, `LOC`, `ORG`, `B-PERSON`, etc. — left
//! untranslated so the adapter recognizer in `nvisy-ner` can apply
//! its `LabelMap` and `labels_to_ignore` policy uniformly across
//! engines.
//!
//! Score is the model's confidence in the raw `[0.0, 1.0]` range;
//! adapters re-clamp to [`Confidence`]
//! and may demote via the configured `low_confidence_score_multiplier`.
//!
//! [`Confidence`]: crate::primitive::Confidence

use std::ops::Range;

/// One raw entity span predicted by a NER model.
///
/// Pre-normalization: the label is the model's string, not a
/// translated [`EntityKind`].
/// Coordinate space is byte offsets into the same source text the
/// surrounding [`NlpArtifacts`] covers.
///
/// [`EntityKind`]: crate::entity::EntityKind
/// [`NlpArtifacts`]: super::NlpArtifacts
#[derive(Debug, Clone, PartialEq)]
pub struct RawNerSpan {
    /// Model-emitted label, verbatim.
    pub label: String,
    /// Model-emitted confidence, in `[0.0, 1.0]`. Out-of-range
    /// values are the producer's bug to fix; consumers clamp.
    pub score: f64,
    /// Byte range of the prediction in the source text.
    pub offset: Range<usize>,
}

impl RawNerSpan {
    /// Construct a span.
    pub fn new(label: impl Into<String>, score: f64, offset: Range<usize>) -> Self {
        Self {
            label: label.into(),
            score,
            offset,
        }
    }
}
