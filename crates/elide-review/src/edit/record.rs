//! [`Edit`]: one change a reviewer makes to an analyzed document.
//!
//! Four operations, in two groups.
//!
//! [`Add`] records a detection recognition missed. It is not a
//! judgement about a detection — it *is* one, sourced from a human
//! instead of a recognizer, so it carries no entity id and lands on
//! the report beside the automatic hits.
//!
//! [`Retag`], [`Suppress`], and [`Redact`] each name an existing
//! entity and override what the policy set decided for it. Only
//! `Redact` is something elide has no concept of: apply re-resolves
//! operators from live policy, because an `OperatorId` carries type
//! and version but no configuration, so an operator override is a
//! governance decision rather than an engine one.
//!
//! # Composing
//!
//! Edits are a list rather than one-per-entity, because they feed
//! independent channels: `Retag` corrects *what a detection is*,
//! while `Suppress`/`Redact` decide *what happens to it*. Retagging
//! an entity and choosing its operator are both legitimate at once.
//!
//! Within one channel, two different answers are a contradiction
//! rather than a refinement, so [`EditSet::validate`] rejects them
//! instead of letting one silently win. Composable pairs merge:
//! two retags setting different fields become one, and a repeated
//! suppress is a duplicate rather than a conflict.
//!
//! [`Add`]: Edit::Add
//! [`Redact`]: Edit::Redact
//! [`Retag`]: Edit::Retag
//! [`Suppress`]: Edit::Suppress

use std::fmt::Write as _;

use elide::entity::LabelRef;
use elide::{Error, ErrorKind};
use elide_governance::modality::RedactableModality;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One change to an analyzed document.
///
/// `Add` carries no id because the entity does not exist yet; the
/// engine mints one when the edit is applied, so a client cannot
/// collide with a real detection or shadow an existing entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "camelCase")]
#[serde(bound = "M::Redaction: Serialize + for<'a> Deserialize<'a>, \
                  M::Location: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Redaction: JsonSchema, M::Location: JsonSchema")]
#[schemars(rename = "{M}Edit")]
pub enum Edit<M: RedactableModality> {
    /// A detection recognition missed.
    Add(Add<M>),
    /// A correction to what an existing detection is or covers.
    Retag(Retag<M>),
    /// A false positive, to be left alone.
    Suppress(Suppress),
    /// An operator to run instead of the policy's pick.
    Redact(Redact<M>),
}

/// A detection recognition missed.
///
/// Recorded on the report with human provenance, so it is never
/// mistaken for an automatic hit, then redacted under the policy set
/// like any other entity: an added label the policy does not cover
/// is left alone.
///
/// Carries no entity id — the entity does not exist yet, and the
/// engine mints one when the edit is applied, so a client cannot
/// collide with a real detection or shadow an existing entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Location: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Location: JsonSchema")]
#[schemars(rename = "{M}Add")]
pub struct Add<M: RedactableModality> {
    /// What the reviewer says this is.
    pub label: LabelRef,
    /// Where it sits in the document.
    pub location: M::Location,
    /// Who made the call, and why.
    #[serde(flatten)]
    pub by: Reviewer,
}

/// Correct what an existing detection *is* or *covers*, then redact
/// it under the policy set as corrected.
///
/// A reviewer fixing recognition's mistake rather than overriding
/// its consequence. Retagging into a label the policy does not cover
/// leaves the entity alone, exactly as if it had been detected that
/// way — which is why it is auditable rather than a mutable field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Location: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Location: JsonSchema")]
#[schemars(rename = "{M}Retag")]
pub struct Retag<M: RedactableModality> {
    /// The entity being corrected.
    pub id: Uuid,
    /// The corrected label, when recognition got it wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<LabelRef>,
    /// The corrected location, when the span clipped the value or
    /// ran past it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<M::Location>,
    /// Who made the call, and why.
    #[serde(flatten)]
    pub by: Reviewer,
}

/// Leave this entity alone: a reviewer calling it a false positive.
///
/// It keeps its place and its provenance; the redaction pass skips
/// it. Recorded as a `Manual` event, so *who* decided and *why* is
/// auditable rather than the detection silently disappearing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Suppress {
    /// The entity to leave alone.
    pub id: Uuid,
    /// Who made the call, and why.
    #[serde(flatten)]
    pub by: Reviewer,
}

/// Redact this entity with `action` instead of the operator the
/// policy picked.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Redaction: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Redaction: JsonSchema")]
#[schemars(rename = "{M}Redact")]
pub struct Redact<M: RedactableModality> {
    /// The entity to redact.
    pub id: Uuid,
    /// The policy whose authority the reviewer exercises. Must match
    /// a submitted policy's `id`.
    ///
    /// Not only for audit: it picks which per-policy pseudonym vault
    /// and `KeyProvider` the operator resolves against, so an
    /// override using `Pseudonymize` or `HmacHash` stays consistent
    /// with that policy's other rules.
    pub policy_id: Uuid,
    /// The operator to run, typed to the entity's own modality so a
    /// text entity cannot be given an image operator.
    pub action: M::Redaction,
    /// Who made the call, and why.
    #[serde(flatten)]
    pub by: Reviewer,
}

/// Who made an edit, and why.
///
/// Shared by all four operations: the reviewer's identity is not
/// part of the decision, so it does not vary with the decision.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Reviewer {
    /// The rationale, when one was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Who made it, when the caller said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

impl<M: RedactableModality> Edit<M> {
    /// The entity this edit names, or `None` for an [`Add`], whose
    /// entity does not exist yet.
    ///
    /// [`Add`]: Edit::Add
    #[must_use]
    pub fn target(&self) -> Option<Uuid> {
        match self {
            Self::Add(_) => None,
            Self::Retag(e) => Some(e.id),
            Self::Suppress(e) => Some(e.id),
            Self::Redact(e) => Some(e.id),
        }
    }

    /// Who made this edit, and why.
    #[must_use]
    pub fn by(&self) -> &Reviewer {
        match self {
            Self::Add(e) => &e.by,
            Self::Retag(e) => &e.by,
            Self::Suppress(e) => &e.by,
            Self::Redact(e) => &e.by,
        }
    }

    /// The wire name of this edit's operation.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Add(_) => "add",
            Self::Retag(_) => "retag",
            Self::Suppress(_) => "suppress",
            Self::Redact(_) => "redact",
        }
    }

    /// Which channel this edit writes to.
    ///
    /// Two edits sharing a channel are two answers to one question;
    /// two on different channels compose.
    #[must_use]
    pub(crate) fn channel(&self) -> Channel {
        match self {
            Self::Add(_) => Channel::Add,
            Self::Retag(_) => Channel::Identity,
            Self::Suppress(_) | Self::Redact(_) => Channel::Outcome,
        }
    }

    /// Why the reviewer made this edit.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.by().reason.as_deref()
    }

    /// Who made this edit.
    #[must_use]
    pub fn actor(&self) -> Option<&str> {
        self.by().actor.as_deref()
    }
}

/// What an edit decides about an entity.
///
/// Edits on different channels compose; two on the same channel are
/// a contradiction unless they merge cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Channel {
    /// Records a new detection. Never conflicts: each names its own
    /// entity.
    Add,
    /// What the detection *is*: its label and span.
    Identity,
    /// What *happens* to it: skipped, or redacted with an operator.
    Outcome,
}

impl<M: RedactableModality> Edit<M> {
    /// Whether `later` refines this edit rather than disagreeing
    /// with it.
    ///
    /// Only asked of two edits on the same channel and entity;
    /// different channels always compose.
    pub(crate) fn merges_with(&self, later: &Self) -> bool {
        match (self, later) {
            // Two retags compose when they set disjoint fields: one
            // fixing the label and one fixing the span are a single
            // correction split across two edits. Both setting the
            // same field are two answers.
            (Self::Retag(a), Self::Retag(b)) => {
                let same_label = a.label.is_some() && b.label.is_some();
                let same_location = a.location.is_some() && b.location.is_some();
                !same_label && !same_location
            }
            // The same call made twice. A retrying client is not a
            // contradicting one.
            (Self::Suppress(_), Self::Suppress(_)) => true,
            // Two operators, or an operator against a suppression:
            // no midpoint exists.
            _ => false,
        }
    }

    /// The error this edit and `later` raise together, naming both
    /// so the caller can see which pair to reconcile.
    pub(crate) fn conflict_with(&self, later: &Self, id: Uuid) -> Error {
        let mut message = format!(
            "{} entity `{id}` carries contradictory edits: `{}` and `{}` answer \
             the same question differently. Send one.",
            M::NAME,
            self.name(),
            later.name(),
        );
        // Actors, when the payload named them: a reviewer
        // reconciling this wants to know who disagreed.
        if let (Some(a), Some(b)) = (self.actor(), later.actor()) {
            let _ = write!(message, " (from `{a}` and `{b}`)");
        }
        Error::new(ErrorKind::Configuration, message)
    }
}
