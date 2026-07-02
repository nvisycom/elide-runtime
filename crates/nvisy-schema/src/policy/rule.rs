//! One rule inside a [`Policy`]: shared identity/description
//! fields, the match predicate, and the action ([`RuleAction`]).
//!
//! The engine compiles a rule's [`predicate`](Rule::predicate)
//! into an elide anonymizer rule at request time. Three shapes
//! are recognised as fast paths and route to the matching
//! [`Anonymizer`] builder method; everything else compiles to a
//! catalog-aware closure:
//!
//! - [`Predicate::LabelOneOf`] with a single label → [`Anonymizer::with_label`]
//! - [`Predicate::TagOneOf`] with a single tag → [`Anonymizer::with_tag`]
//! - any composite (or multi-label / multi-tag) → [`Anonymizer::with_catalog_predicate`]
//!
//! Author-facing wire format: rules carry one composable
//! `predicate` field. No need for separate label/tag/predicate
//! kinds: `Predicate::LabelOneOf { labels: ["email"] }` is the
//! same rule as the old `RuleKind::Label { label: "email" }` and
//! compiles down to the same fast path.
//!
//! [`Policy`]: super::Policy
//! [`Anonymizer`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer
//! [`Anonymizer::with_label`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_label
//! [`Anonymizer::with_tag`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_tag
//! [`Anonymizer::with_catalog_predicate`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_catalog_predicate

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::audit::AuditAction;
use super::predicate::Predicate;
use super::redaction::ModalityRedactions;
use super::suppress::SuppressAction;

/// One rule inside a [`Policy`]. Identity is the UUID; `name` /
/// `description` are display-only.
///
/// [`Policy`]: super::Policy
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// Stable identifier. UUIDv7 recommended (time-ordered);
    /// customer-supplied so re-submissions carry the same id.
    /// Engine stamps this into the redaction event's
    /// [`Attribution::reason`] so reviewers can trace back which
    /// rule fired.
    ///
    /// [`Attribution::reason`]: elide_core::entity::provenance::Attribution::reason
    pub id: Uuid,
    /// Human-readable name. Display-only. Does not key anything.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Entity-level predicate that decides whether the rule fires
    /// on a given recognised entity. Composable; see
    /// [`Predicate`] for the full grammar.
    pub predicate: Predicate,
    /// What to do on match.
    pub action: RuleAction,
}

/// What a rule does when its [`predicate`](Rule::predicate)
/// matches.
///
/// Three verbs: [`Redact`](RuleAction::Redact) transforms the
/// entity with one operator per modality;
/// [`Suppress`](RuleAction::Suppress) drops the entity entirely
/// (false-positive marker) and stamps a reason onto the audit;
/// [`Audit`](RuleAction::Audit) flags it for human review without
/// transforming.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleAction {
    /// Redact matching entities. The carried map names operators
    /// per modality; modalities the rule didn't cover fall through
    /// to the policy fallback (or the next policy in the chain).
    Redact(ModalityRedactions),
    /// Suppress matching entities: treat as a false positive and
    /// skip redaction. The audit records the suppression.
    Suppress(SuppressAction),
    /// Flag matching entities for human review. The entity is
    /// left untouched; the audit entry carries the severity hint
    /// for downstream tooling.
    Audit(AuditAction),
}
