//! [`ConflictResolution`]: how to resolve cross-kind span overlaps.

use std::cmp::Ordering;

use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Modality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::deduplication::span_size::SpanSize;

/// How to resolve conflicts when different entity kinds overlap the
/// same text span.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Keep only the entity with the highest confidence score.
    #[default]
    HighestConfidence,
    /// Keep only the entity with the highest sensitivity level.
    HighestSensitivity,
    /// Keep only the entity with the longest span (most specific match).
    LongestSpan,
}

impl ConflictResolution {
    /// `true` when `a` wins the overlap against `b` under this
    /// strategy.
    pub(super) fn keeps_first<M>(&self, a: &Entity<M>, b: &Entity<M>) -> bool
    where
        M: Modality + SpanSize,
    {
        match self {
            Self::HighestConfidence => a.confidence.get() >= b.confidence.get(),
            Self::HighestSensitivity => match (a.sensitivity, b.sensitivity) {
                (Some(sa), Some(sb)) => sa >= sb,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => a.confidence.get() >= b.confidence.get(),
            },
            Self::LongestSpan => {
                a.location.span_cmp(&b.location).unwrap_or(Ordering::Equal) != Ordering::Less
            }
        }
    }
}
