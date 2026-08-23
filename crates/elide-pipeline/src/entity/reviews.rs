//! [`ReviewSet`]: the reviewer decisions attached to a report,
//! kept typed per modality and keyed by entity id.
//!
//! A [`Report`] holds detections; elide owns those. What it has no
//! concept of is a reviewer *changing the operator* that hides one:
//! [`anonymize_with`] re-resolves operators from live policy at
//! apply time, deliberately, because an `OperatorId` carries type
//! and version but no configuration and operators are not
//! serializable. So an operator override is a governance decision,
//! and it lives here beside the report rather than inside it.
//!
//! Keyed by [`Entity::id`], a stable UUIDv7 that survives
//! serialization, so a decision made in one request still finds its
//! entity when a host posts the audit back.
//!
//! One map per modality rather than one keyed map of type-erased
//! decisions: a [`Review<M>`] names an `M::Redaction`, so erasing
//! it would let a reviewer attach an image redaction to a text
//! entity again — the mismatch the typed review exists to prevent.
//!
//! [`Entity::id`]: elide::entity::Entity::id
//! [`Report`]: elide::Report
//! [`anonymize_with`]: elide::Orchestrator::anonymize_with

use std::collections::HashMap;

use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide_governance::modality::RedactableModality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::record::Review;

/// Reviewer decisions for one request, bucketed by the modality of
/// the entity each one targets.
///
/// Empty buckets are omitted on the wire, so an audit with no
/// reviews serializes as `{}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSet {
    /// Decisions on text entities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub text: HashMap<Uuid, Review<Text>>,
    /// Decisions on tabular entities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tabular: HashMap<Uuid, Review<Tabular>>,
    /// Decisions on image entities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub image: HashMap<Uuid, Review<Image>>,
    /// Decisions on audio entities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub audio: HashMap<Uuid, Review<Audio>>,
}

impl ReviewSet {
    /// Whether no entity carries a decision.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.tabular.is_empty()
            && self.image.is_empty()
            && self.audio.is_empty()
    }

    /// How many entities carry a decision, across every modality.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len() + self.tabular.len() + self.image.len() + self.audio.len()
    }
}

/// The bucket of [`ReviewSet`] that holds decisions for modality
/// `Self`.
///
/// Lets code generic over `M` reach the right map without a
/// four-arm match at every call site. Implemented for exactly the
/// four modalities [`ReviewSet`] has buckets for.
pub trait ReviewBucket: RedactableModality + Sized {
    /// This modality's decisions, keyed by entity id.
    fn bucket(set: &ReviewSet) -> &HashMap<Uuid, Review<Self>>;

    /// The same bucket, for recording a decision.
    fn bucket_mut(set: &mut ReviewSet) -> &mut HashMap<Uuid, Review<Self>>;
}

macro_rules! impl_review_bucket {
    ($($modality:ty => $field:ident),+ $(,)?) => {
        $(
            impl ReviewBucket for $modality {
                fn bucket(set: &ReviewSet) -> &HashMap<Uuid, Review<Self>> {
                    &set.$field
                }

                fn bucket_mut(set: &mut ReviewSet) -> &mut HashMap<Uuid, Review<Self>> {
                    &mut set.$field
                }
            }
        )+
    };
}

impl_review_bucket! {
    Text => text,
    Tabular => tabular,
    Image => image,
    Audio => audio,
}
