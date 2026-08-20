//! One recognised entity plus the optional reviewer override.

use elide::entity::Entity;
use elide::modality::Modality;
use elide_governance::redaction::ModalityRedactions;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One recognized entity plus the optional reviewer override.
///
/// The bound mirrors elide's [`Entity<M>`]: serialization needs
/// `M::Location` and `M::Data` (de)serializable, and JsonSchema
/// derivation needs them schema-able. All four modalities elide
/// ships satisfy these under the `serde` + `schema` features.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Location: Serialize + for<'a> Deserialize<'a>, \
                  M::Data: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Location: JsonSchema, M::Data: JsonSchema")]
#[schemars(rename = "{M}EntityRecord")]
pub struct EntityRecord<M: Modality> {
    /// The elide entity, as recognition produced it.
    pub entity: Entity<M>,
    /// Reviewer-supplied redaction override.
    ///
    /// `None` means "use the matching policy rule's decision";
    /// `Some(...)` overrides that rule for this specific entity
    /// at apply time. Reviewer overrides take precedence over
    /// every policy rule and inherit the authority of the
    /// [`Review::policy_id`] they name: the audit event's
    /// attribution stamps that policy so the trail names the
    /// authority under which the override fired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<Review>,
}

/// A reviewer-supplied redaction override with the policy
/// authority it draws from.
///
/// The `policy_id` isn't just for audit: it also picks which
/// per-policy pseudonym vault and per-policy [`KeyProvider`] the
/// override's operator resolves against, so an override using
/// [`Pseudonymize`] or [`HmacHash`] stays consistent with the
/// authoring policy's other rules.
///
/// [`KeyProvider`]: elide::redaction::operators::KeyProvider
/// [`Pseudonymize`]: elide::redaction::operators::Pseudonymize
/// [`HmacHash`]: elide::redaction::operators::HmacHash
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// The policy whose authority the reviewer exercises. Must
    /// match the `id` of a [`PolicyDefinition`] submitted with
    /// the anonymize request. The audit event stamps this UUID
    /// as the attribution `name`.
    ///
    /// [`PolicyDefinition`]: elide_governance::PolicyDefinition
    pub policy_id: Uuid,
    /// The per-modality redaction operators to run for this
    /// entity. Overrides whatever the policy set would have
    /// picked for the same entity.
    pub action: ModalityRedactions,
}

impl<M: Modality> EntityRecord<M> {
    /// New record over `entity`, no review override.
    pub fn new(entity: Entity<M>) -> Self {
        Self {
            entity,
            review: None,
        }
    }
}

/// The three fields the anonymize path needs to apply a reviewer
/// override: which entity to target, which policy's authority
/// the override draws from, and the operator spec to run.
///
/// Materialised from every [`EntityRecord::review`] in the audit
/// and consumed by the anonymizer to layer reviewer decisions
/// before policy rules.
#[derive(Debug, Clone)]
pub struct OverrideEntry {
    /// The entity the reviewer overrode.
    pub entity_id: Uuid,
    /// The policy whose authority the override exercises. Named
    /// on the audit event's attribution and used to look up any
    /// per-policy operator infrastructure the override pulls in.
    pub policy_id: Uuid,
    /// The per-modality operator spec to run.
    pub action: ModalityRedactions,
}
