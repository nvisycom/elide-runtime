//! One recognised entity plus the optional reviewer override.
//!
//! A reviewer can do three things to a detection, and each has
//! exactly one home:
//!
//! - **suppress** it (a false positive) — [`EntityRecord::suppress`],
//!   recorded as a [`Manual`] event on the entity's own trail, which
//!   is also what elide reads to skip it at apply time.
//! - **add** one recognition missed — [`EntityGroup::include`], same
//!   mechanism, opposite intent.
//! - **change the operator** that hides it — [`EntityRecord::review`].
//!
//! The first two live on the audit trail because elide owns the
//! semantics: a suppressed entity stays in the report, keeps its
//! provenance, and the redaction pass skips it, so nothing vanishes
//! without a record of who decided and why. The third cannot live
//! there: elide re-resolves the operator from live policy rules at
//! apply time, deliberately, because an [`OperatorId`] carries type
//! and version but no configuration, and operators are not
//! serializable. So an operator change is a governance decision,
//! carried here against the policy whose authority it draws from.
//!
//! [`EntityGroup::include`]: super::EntityGroup::include
//! [`Manual`]: elide::entity::audit::AuditKind::Manual
//! [`OperatorId`]: elide::entity::audit::OperatorId

use elide::entity::{Entity, LabelRef};
use elide_governance::modality::RedactableModality;
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
pub struct EntityRecord<M: RedactableModality> {
    /// The elide entity, as recognition produced it, plus whatever
    /// an applied review corrected.
    ///
    /// Read-only: reach it through [`entity`](Self::entity). Every
    /// change goes through [`review`](Self::review), so a correction
    /// leaves a [`Manual`] event on the trail. Mutating the label or
    /// location directly would let a retag out of a policy's scope
    /// silently drop the entity from redaction with nothing recorded,
    /// which is a suppression that never had to admit it was one.
    ///
    /// [`Manual`]: elide::entity::audit::AuditKind::Manual
    pub(crate) entity: Entity<M>,
    /// The reviewer's decision about this detection.
    ///
    /// `None` means "whatever the policy picked" — the
    /// [`Selection`] already on the entity's trail. `Some` replaces
    /// it at apply time, either redacting differently
    /// ([`Review::Redact`]) or not at all ([`Review::Suppress`]).
    ///
    /// Set it through [`redact`](Self::redact) or
    /// [`suppress`](Self::suppress) rather than assigning
    /// directly; both replace any decision already held, since one
    /// entity carries one decision.
    ///
    /// A `Redact` takes precedence over every policy rule and
    /// inherits the authority of the policy it names: the audit
    /// event's attribution stamps that policy, so the trail records
    /// which authority the override fired under.
    ///
    /// [`Selection`]: elide::entity::audit::AuditKind::Selection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<Review<M>>,
}

/// A reviewer's decision about one detection, overriding what
/// the policy set picked for it.
///
/// The two arms are the two things a reviewer can decide about an
/// entity that *was* detected, and they are mutually exclusive by
/// construction: an entity is either redacted differently or not
/// redacted at all. As separate fields they could both be set, and
/// one would silently win. (Adding an entity recognition missed is
/// the third action and lives on [`EntityGroup::include`], since
/// there is no record yet to attach a decision to.)
///
/// The engine's own pick is on the entity's trail as a
/// [`Selection`] before any of this: a reviewer reads *what would
/// happen and why*, then overrides it here.
///
/// [`EntityGroup::include`]: super::EntityGroup::include
/// [`Selection`]: elide::entity::audit::AuditKind::Selection
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "camelCase")]
#[serde(bound = "M::Redaction: Serialize + for<'a> Deserialize<'a>, \
                  M::Location: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Redaction: JsonSchema, M::Location: JsonSchema")]
#[schemars(rename = "{M}Review")]
pub enum Review<M: RedactableModality> {
    /// Redact this entity with `action` instead of the operator
    /// the policy picked.
    #[serde(rename_all = "camelCase")]
    Redact {
        /// The policy whose authority the reviewer exercises. Must
        /// match the `id` of a [`PolicyDefinition`] submitted with
        /// the anonymize request. The audit event stamps this UUID
        /// as the attribution `name`.
        ///
        /// Not only for audit: it also picks which per-policy
        /// pseudonym vault and per-policy [`KeyProvider`] the
        /// operator resolves against, so an override using
        /// [`Pseudonymize`] or [`HmacHash`] stays consistent with
        /// the authoring policy's other rules.
        ///
        /// [`KeyProvider`]: elide::redaction::operators::KeyProvider
        /// [`PolicyDefinition`]: elide_governance::PolicyDefinition
        /// [`Pseudonymize`]: elide::redaction::operators::Pseudonymize
        /// [`HmacHash`]: elide::redaction::operators::HmacHash
        policy_id: Uuid,
        /// The redaction operator to run for this entity.
        ///
        /// Typed to the record's own modality, so a review on an
        /// `EntityRecord<Text>` can only name a [`TextRedaction`].
        /// The four-slot [`ModalityRedactions`] a policy rule
        /// carries would let a reviewer declare an image operator
        /// here and have it silently discarded at apply time.
        ///
        /// [`ModalityRedactions`]: elide_governance::redaction::ModalityRedactions
        /// [`TextRedaction`]: elide_governance::redaction::TextRedaction
        action: M::Redaction,
    },
    /// Correct what this detection *is* or *covers*, then redact it
    /// under the policy set as corrected.
    ///
    /// A reviewer fixing recognition's mistake rather than
    /// overriding its consequence: a label the recognizer got wrong,
    /// or a span that clipped the value short. The policy set is
    /// then re-applied to the corrected entity, so a retag into a
    /// label the policy does not cover leaves the entity alone,
    /// exactly as if it had been detected that way.
    ///
    /// That last case is why this is a variant rather than a
    /// mutable field. Retagging out of a policy's scope silently
    /// drops an entity from redaction, which is a suppression in
    /// everything but name; routing it through here records a
    /// [`Manual`] event so the decision is auditable like any other.
    ///
    /// [`Manual`]: elide::entity::audit::AuditKind::Manual
    #[serde(rename_all = "camelCase")]
    Retag {
        /// The corrected label, when recognition got it wrong.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<LabelRef>,
        /// The corrected location, when the detected span clipped
        /// the value or ran past it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<M::Location>,
        /// Why the correction was made.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// Who made it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actor: Option<String>,
    },
    /// Leave this entity alone: a reviewer calling it a false
    /// positive.
    ///
    /// It keeps its place in the audit and its provenance; the
    /// redaction pass skips it. Recorded on the entity's own trail
    /// as a [`Manual`] event carrying `reason` and `actor`, so
    /// *who* decided and *why* is auditable rather than the
    /// detection silently disappearing.
    ///
    /// [`Manual`]: elide::entity::audit::AuditKind::Manual
    #[serde(rename_all = "camelCase")]
    Suppress {
        /// Why the reviewer judged this a false positive.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// Who made the call.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        actor: Option<String>,
    },
}

impl<M: RedactableModality> EntityRecord<M> {
    /// The detection this record wraps.
    #[must_use]
    pub fn entity(&self) -> &Entity<M> {
        &self.entity
    }

    /// Correct what this detection is or covers, then let the policy
    /// set decide again with the correction in place.
    ///
    /// For recognition's mistakes: a wrong label, or a span that
    /// clipped the value. `None` leaves that part as detected.
    /// Replaces any decision already held.
    ///
    /// A retag into a label the policy set does not cover leaves the
    /// entity unredacted, which is the honest consequence of saying
    /// it was never that kind of value. The [`Manual`] event this
    /// records keeps that decision auditable.
    ///
    /// [`Manual`]: elide::entity::audit::AuditKind::Manual
    pub fn retag(
        &mut self,
        label: Option<LabelRef>,
        location: Option<M::Location>,
        reason: Option<String>,
        actor: Option<String>,
    ) {
        self.review = Some(Review::Retag {
            label,
            location,
            reason,
            actor,
        });
    }

    /// New record over `entity`, no review override.
    pub fn new(entity: Entity<M>) -> Self {
        Self {
            entity,
            review: None,
        }
    }

    /// Mark this detection as one to leave alone: a reviewer
    /// calling it a false positive.
    ///
    /// Records the decision as [`Review::Suppress`]; the redaction
    /// pass skips the entity and stamps a [`Manual`] event on its
    /// trail carrying `reason` and `actor`, so *who* decided and
    /// *why* is auditable rather than the detection silently
    /// disappearing.
    ///
    /// Replaces any operator override already set: the two are one
    /// decision, and an entity that is not redacted has no operator
    /// to run.
    ///
    /// [`Manual`]: elide::entity::audit::AuditKind::Manual
    pub fn suppress(&mut self, reason: Option<String>, actor: Option<String>) {
        self.review = Some(Review::Suppress { reason, actor });
    }

    /// Redact this entity with `action` instead of the operator
    /// the policy picked, under `policy_id`'s authority.
    ///
    /// Replaces any suppression already set, for the same reason
    /// [`suppress`](Self::suppress) replaces an override.
    pub fn redact(&mut self, policy_id: Uuid, action: M::Redaction) {
        self.review = Some(Review::Redact { policy_id, action });
    }

    /// Whether this detection will be left alone.
    ///
    /// A pending decision wins over what the trail already records,
    /// so a reviewer who suppressed an entity, applied, then changed
    /// their mind and called [`redact`](Self::redact) reads `false`
    /// here — and the entity is redacted on the next apply. Reading
    /// the trail alone would answer with the decision that has been
    /// superseded.
    ///
    /// With no pending decision this falls back to the trail, so an
    /// applied suppression still reads as suppressed after an audit
    /// round-trip.
    #[must_use]
    pub fn is_suppressed(&self) -> bool {
        match self.review {
            Some(Review::Suppress { .. }) => true,
            // A retag is re-detection, not a decision to skip: the
            // corrected entity goes back through the policy set.
            Some(Review::Redact { .. }) | Some(Review::Retag { .. }) => false,
            None => self.entity.is_suppressed(),
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
pub struct OverrideEntry<M: RedactableModality> {
    /// The entity the reviewer overrode.
    pub entity_id: Uuid,
    /// The policy whose authority the override exercises. Named
    /// on the audit event's attribution and used to look up any
    /// per-policy operator infrastructure the override pulls in.
    pub policy_id: Uuid,
    /// The operator spec to run, typed to the modality of the
    /// entity it targets.
    pub action: M::Redaction,
}
