//! [`Override`]: one reviewer decision to redact an entity
//! differently from what the policy set picked.
//!
//! A provider compiles policies into anonymizers, and a reviewer
//! may overrule the operator chosen for one entity. That is all the
//! provider needs to know: which entity, whose authority, which
//! operator. *Why* a reviewer decided so, and what else they might
//! have decided instead, belongs to whoever holds the audit.
//!
//! Typed per modality, so an override on a text entity can only
//! name a [`TextRedaction`]: a redaction spec and the entity it
//! applies to can never disagree about what medium they are in.
//!
//! [`TextRedaction`]: elide_governance::redaction::TextRedaction

use std::collections::HashMap;

use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::modality::tabular::Tabular;
use elide::modality::text::Text;
use elide_governance::modality::RedactableModality;
use uuid::Uuid;

/// A reviewer's operator choice for one entity.
#[derive(Debug, Clone)]
pub struct Override<M: RedactableModality> {
    /// The policy whose authority the reviewer exercises.
    ///
    /// Not only for the audit trail: it also picks which per-policy
    /// pseudonym vault and [`KeyProvider`] the operator resolves
    /// against, so an override using `Pseudonymize` or `HmacHash`
    /// stays consistent with the authoring policy's other rules.
    ///
    /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
    pub policy_id: Uuid,
    /// The operator to run instead of the policy's pick.
    pub action: M::Redaction,
}

/// Reviewer overrides for one request, bucketed by the modality of
/// the entity each one targets and keyed by entity id.
///
/// One map per modality rather than one map of type-erased
/// overrides: an [`Override<M>`] names an `M::Redaction`, so erasing
/// it would let an image redaction attach to a text entity.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// Overrides on text entities.
    pub text: HashMap<Uuid, Override<Text>>,
    /// Overrides on tabular entities.
    pub tabular: HashMap<Uuid, Override<Tabular>>,
    /// Overrides on image entities.
    pub image: HashMap<Uuid, Override<Image>>,
    /// Overrides on audio entities.
    pub audio: HashMap<Uuid, Override<Audio>>,
}

impl Overrides {
    /// Every override's `(entity_id, policy_id)` pair.
    ///
    /// Validation only needs the authority a reviewer named, not
    /// the operator they picked, so this stays modality-agnostic
    /// and spares the caller four near-identical loops.
    pub(crate) fn authorities(&self) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
        authorities_of(&self.text)
            .chain(authorities_of(&self.tabular))
            .chain(authorities_of(&self.image))
            .chain(authorities_of(&self.audio))
    }
}

/// The `(entity_id, policy_id)` pair of every override in one
/// modality's bucket.
///
/// A free generic fn rather than a closure: each bucket has a
/// different `M`, and a closure cannot be generic over it.
fn authorities_of<M: RedactableModality>(
    overrides: &HashMap<Uuid, Override<M>>,
) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
    overrides.iter().map(|(id, o)| (*id, o.policy_id))
}
