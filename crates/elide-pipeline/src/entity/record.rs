//! [`Review`]: what a reviewer decided about one detection.
//!
//! A reviewer can do four things to a document, and each has
//! exactly one home:
//!
//! - **suppress** a false positive — [`Review::Suppress`]
//! - **correct** a wrong label or a clipped span —
//!   [`Review::Retag`]
//! - **change the operator** that hides it — [`Review::Redact`]
//! - **add** a detection recognition missed — elide's own
//!   [`Report::include`]
//!
//! The first three live here because they override what the policy
//! set decided, and one of them — changing the operator — is
//! something elide has no concept of: apply re-resolves operators
//! from live policy, because an `OperatorId` carries type and
//! version but no configuration and operators are not
//! serializable. So an operator override is a governance decision.
//!
//! Adding an entity is elide's, not ours: there is no policy
//! decision to override, only a detection to record, and elide
//! stamps the human provenance itself.
//!
//! [`Report::include`]: elide::Report::include

use elide::entity::LabelRef;
use elide_governance::modality::RedactableModality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A reviewer's decision about one detection, overriding what
/// the policy set picked for it.
///
/// The two arms are the two things a reviewer can decide about an
/// entity that *was* detected, and they are mutually exclusive by
/// construction: an entity is either redacted differently, or
/// corrected, or not redacted at all. As separate fields they could
/// both be set, and one would silently win. (Adding an entity
/// recognition missed is elide's own [`Report::include`]: there is
/// no policy decision to override.)
///
/// The engine's own pick is on the entity's trail as a
/// [`Selection`] before any of this: a reviewer reads *what would
/// happen and why*, then overrides it here.
///
/// [`Report::include`]: elide::Report::include
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
        /// Typed to the entity's own modality, so
        /// a `Review<Text>` can only name a [`TextRedaction`].
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

/// The three fields the anonymize path needs to apply a reviewer
/// override: which entity to target, which policy's authority
/// the override draws from, and the operator spec to run.
///
/// Materialised from every [`Review::Redact`] in the audit and
/// consumed by the anonymizer to layer reviewer decisions before
/// policy rules.
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
