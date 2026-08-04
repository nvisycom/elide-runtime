//! One rule inside a [`PolicyDefinition`]: shared identity/description
//! fields, the match predicate, and the redaction operators
//! ([`ModalityRedactions`]) to apply on match.
//!
//! The engine compiles a rule's [`predicate`] into an elide
//! anonymizer rule at request time. Three shapes are recognised
//! as fast paths and route to the matching [`Anonymizer`] builder
//! method; everything else compiles to a catalog-aware closure:
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
//! [`PolicyDefinition`]: super::PolicyDefinition
//! [`predicate`]: PolicyRule::predicate
//! [`Anonymizer`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer
//! [`Anonymizer::with_label`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_label
//! [`Anonymizer::with_tag`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_tag
//! [`Anonymizer::with_catalog_predicate`]: https://docs.rs/elide/latest/elide/redaction/Anonymizer::with_catalog_predicate

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::predicate::Predicate;
use super::redaction::ModalityRedactions;

/// One rule inside a [`PolicyDefinition`]. Identity is the UUID; `name` /
/// `description` are display-only.
///
/// [`PolicyDefinition`]: super::PolicyDefinition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
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
    /// Per-modality redaction operators applied when the
    /// predicate matches. Modalities the rule doesn't cover fall
    /// through to the policy fallback (or the next policy in the
    /// chain).
    pub action: ModalityRedactions,
}
