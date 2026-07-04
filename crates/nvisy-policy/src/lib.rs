#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Architecture
//!
//! Authored vocabulary for redaction governance.
//!
//! A request submits `Vec<Policy>` in precedence order. Engine
//! walks them; for each policy whose [`Policy::applies_when`]
//! holds against the document, it walks [`Policy::rules`] in
//! order and runs the first matching rule's [`PolicyAction`]. If
//! no rule in a policy matches, the policy's [`Policy::fallback`]
//! runs (and the chain halts) if set; otherwise the engine moves
//! to the next policy. If no policy matches and no policy
//! carries a fallback, the entity is skipped.
//!
//! Identity is UUID-keyed: every [`Policy`] and every [`PolicyRule`]
//! carries a stable [`Uuid`]. Engine stamps `policy.id` and
//! `rule.id` into the redaction event's [`Attribution`] so
//! reviewers can trace any redaction back to the exact rule that
//! fired.
//!
//! [`Attribution`]: elide_core::entity::provenance::Attribution

mod action;
pub mod predicate;
pub mod redaction;
pub mod retention;
mod rule;

use elide_core::entity::Label;
use hipstr::HipStr;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use self::action::{AuditAction, SuppressAction};
use self::predicate::DocumentPredicate;
use self::retention::RetentionPolicy;
pub use self::rule::{PolicyAction, PolicyRule};

/// A named, versioned governance policy.
///
/// Identity is the UUID; `name` is display-only. `version` is the
/// policy body's semver: two submissions of the same
/// `(id, version)` pair should produce identical decisions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// Stable identifier. UUIDv7 recommended (time-ordered);
    /// customer-supplied so re-submissions carry the same id.
    /// Engine stamps this into the redaction event's
    /// [`Attribution::policy_id`] so reviewers can find this
    /// policy from any redaction it drove.
    ///
    /// [`Attribution::policy_id`]: elide_core::entity::provenance::Attribution::policy_id
    pub id: Uuid,
    /// Human-readable name. Display-only. Does not key anything.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Semver of the policy body.
    #[schemars(with = "String")]
    pub version: Version,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Document-level gate. The whole policy (rules + fallback)
    /// is skipped when this is `Some(...)` and the predicate is
    /// false for the document. Evaluated once per document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_when: Option<DocumentPredicate>,
    /// Vocabulary the policy operates over. Engine unions every
    /// submitted policy's `labels` into a per-request
    /// [`LabelCatalog`] used to drive recognizer dispatch and
    /// tag-based [`Predicate::TagOneOf`] matching.
    ///
    /// [`LabelCatalog`]: elide_core::entity::LabelCatalog
    /// [`Predicate::TagOneOf`]: predicate::Predicate::TagOneOf
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<Label>,
    /// Ordered rules. First match wins within this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all. Fires when no rule in this policy
    /// matched. Presence halts the chain; absence falls through
    /// to the next policy. [`Option`] enforces "at most one
    /// fallback per policy" at the type level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<PolicyAction>,
    /// Lifecycle rules for content under this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}
