#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Architecture
//!
//! Authored vocabulary for redaction governance.
//!
//! A request submits `Vec<PolicyDefinition>` in precedence order.
//! Engine walks the policies; for each policy whose
//! [`PolicyDefinition::when`] holds against the document, it walks
//! [`PolicyDefinition::rules`] in order and runs the first matching
//! rule's redaction operators. If no rule in a policy matches, the
//! policy's [`PolicyDefinition::fallback`] runs (and the chain
//! halts) if set; otherwise the engine moves to the next policy.
//! If no policy matches and no policy carries a fallback, the
//! entity is skipped.
//!
//! Rules have two shapes ([`PolicyRule`]):
//! - [`Predicated`]: one composable [`Predicate`] gates a single
//!   [`ModalityRedactions`] action.
//! - [`Table`]: a list of per-label [`LabelEntry`] entries — the
//!   compile-time sugar for "route each label to its own operator
//!   under one shared rule identity" (e.g. HIPAA Safe Harbor
//!   fan-out).
//!
//! [`LabelGroup`]s are named clusters of [`LabelRef`]s a
//! [`Predicate::LabelInGroup`] references by name. Groups live
//! on the policy that declares them (`hipaa_safe_harbor` policy
//! carries a `hipaa_18` group); a rule can reference groups its
//! own policy declared, not another policy's. At request-compile
//! time the engine stamps `group:<policy_id>:<name>` tags on the
//! listed labels; unknown group names error at validation.
//!
//! Identity is UUID-keyed: every [`PolicyDefinition`] and every
//! [`PolicyRule`] carries a stable [`Uuid`]. Engine stamps
//! `policy.id` and `rule.id` into the redaction event's
//! [`Attribution`] so reviewers can trace any redaction back to
//! the exact rule that fired.
//!
//! [`Attribution`]: elide_core::entity::provenance::Attribution
//! [`LabelRef`]: elide_core::entity::LabelRef
//! [`ModalityRedactions`]: redaction::ModalityRedactions
//! [`Predicate`]: predicate::Predicate
//! [`Predicate::LabelInGroup`]: predicate::Predicate::LabelInGroup
//! [`Predicated`]: PolicyRule::Predicated
//! [`Table`]: PolicyRule::Table

mod label;
pub mod predicate;
pub mod redaction;
pub mod retention;
mod rule;

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::label::{LabelGroup, Labels};
use self::predicate::DocumentPredicate;
use self::redaction::ModalityRedactions;
use self::retention::RetentionPolicy;
pub use self::rule::{LabelEntry, PolicyRule, PredicatedRule, TableRule};

/// A named governance policy.
///
/// Identity is the UUID; `name` is display-only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDefinition {
    /// Stable identifier. UUIDv7 recommended (time-ordered);
    /// customer-supplied so re-submissions carry the same id.
    /// Engine stamps this into the redaction event's
    /// [`Attribution::name`] so reviewers can find this policy
    /// from any redaction it drove.
    ///
    /// [`Attribution::name`]: elide_core::entity::provenance::Attribution::name
    pub id: Uuid,
    /// Human-readable name. Display-only. Does not key anything.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Document-level gate. The whole policy (rules + fallback)
    /// is skipped when this is `Some(...)` and the predicate is
    /// false for the document. Evaluated once per document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<DocumentPredicate>,
    /// Vocabulary the policy operates over: builtins picked by
    /// name plus caller-authored custom label schemas. Engine
    /// unions every submitted policy's `labels` into a per-request
    /// [`LabelCatalog`] used to drive recognizer dispatch and
    /// tag-based [`Predicate::TagOneOf`] matching.
    ///
    /// [`LabelCatalog`]: elide_core::entity::LabelCatalog
    /// [`Predicate::TagOneOf`]: predicate::Predicate::TagOneOf
    #[serde(default, skip_serializing_if = "Labels::is_empty")]
    pub labels: Labels,
    /// Named clusters of [`LabelRef`]s this policy's rules may
    /// reference by name via [`Predicate::LabelInGroup`]. Scoped
    /// to this policy — a rule can only name a group its own
    /// policy declared; unknown references error at request
    /// validation. Two policies that both declare `hipaa_18` with
    /// different labelsets stay independent.
    ///
    /// [`LabelRef`]: elide_core::entity::LabelRef
    /// [`Predicate::LabelInGroup`]: predicate::Predicate::LabelInGroup
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<LabelGroup>,
    /// Ordered rules. First match wins within this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all. Fires when no rule in this policy
    /// matched. Presence halts the chain; absence falls through
    /// to the next policy. [`Option`] enforces "at most one
    /// fallback per policy" at the type level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<ModalityRedactions>,
    /// Lifecycle rules for content under this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}
