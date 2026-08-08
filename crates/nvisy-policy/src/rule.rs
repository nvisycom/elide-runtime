//! One rule inside a [`PolicyDefinition`]: either predicate-gated
//! (one predicate, one action) or per-label table (a map of label
//! → action for compact fan-out).
//!
//! Both variants carry the same identity/description fields; the
//! shape differs only in how targets are selected. The engine
//! compiles either shape into elide [`Rule`]s attached via
//! [`Anonymizer::with`]:
//!
//! - **Predicated** rule → the `predicate` compiles into an elide
//!   selector. Fast paths:
//!   - [`Predicate::LabelOneOf`] with a single label → [`Rule::label`]
//!   - [`Predicate::TagOneOf`] with a single tag → [`Rule::tag`]
//!   - anything else → [`Rule::predicate`]
//! - **Table** rule → each `(label, action)` entry attaches as
//!   [`Rule::label`] directly; the table is sugar over N
//!   predicated rules with identical id/name/description.
//!
//! Table rules keep templates that need per-label operator
//! dispatch (HIPAA Safe Harbor identifier fan-out: date →
//! generalize-year, age → clamp-≥90, name → erase, …) from
//! ballooning to one predicated rule per label with the same
//! identity boilerplate repeated.
//!
//! Wire format is an untagged serde enum: existing JSON with
//! `predicate` and `action` fields parses as [`PolicyRule::Predicated`];
//! JSON with an `operators` field parses as [`PolicyRule::Table`].
//!
//! [`PolicyDefinition`]: super::PolicyDefinition
//! [`Predicate`]: super::predicate::Predicate
//! [`Predicate::LabelOneOf`]: super::predicate::Predicate::LabelOneOf
//! [`Predicate::TagOneOf`]: super::predicate::Predicate::TagOneOf
//! [`Anonymizer::with`]: elide_redaction::Anonymizer::with
//! [`Rule`]: elide_redaction::Rule
//! [`Rule::label`]: elide_redaction::Rule::label
//! [`Rule::tag`]: elide_redaction::Rule::tag
//! [`Rule::predicate`]: elide_redaction::Rule::predicate

use elide_core::entity::LabelRef;
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::predicate::Predicate;
use super::redaction::ModalityRedactions;

/// One rule inside a [`PolicyDefinition`]. Identity is the UUID;
/// `name` / `description` are display-only.
///
/// Untagged on the wire: distinguished by the presence of
/// `predicate` (predicated) vs. `operators` (table). Existing
/// JSON keeps working.
///
/// [`PolicyDefinition`]: super::PolicyDefinition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PolicyRule {
    /// Predicate-gated: one predicate, one action, fires when the
    /// predicate holds on the candidate entity.
    ///
    /// Boxed because [`PredicatedRule`] carries a recursive
    /// [`Predicate`] tree; without indirection the enum's
    /// stack footprint balloons to the largest variant.
    Predicated(Box<PredicatedRule>),
    /// Per-label table: one action per label, sugar over N
    /// predicated rules with identical id/name/description.
    Table(TableRule),
}

impl PolicyRule {
    /// Stable identifier — display-independent, shared by both
    /// variants. Engine stamps it into the redaction event's
    /// [`Attribution::description`] so reviewers can trace which
    /// rule fired.
    ///
    /// [`Attribution::description`]: elide_core::entity::provenance::Attribution::description
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::Predicated(r) => r.id,
            Self::Table(r) => r.id,
        }
    }

    /// Human-readable name — display only.
    #[must_use]
    pub fn name(&self) -> &HipStr<'static> {
        match self {
            Self::Predicated(r) => &r.name,
            Self::Table(r) => &r.name,
        }
    }

    /// Optional reviewer description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Predicated(r) => r.description.as_deref(),
            Self::Table(r) => r.description.as_deref(),
        }
    }

    /// Flatten this rule into one or more `(predicate, action)`
    /// pairs. Predicated yields one pair verbatim; Table yields
    /// one pair per [`LabelEntry`] with a synthetic
    /// [`Predicate::LabelOneOf`] holding that single label. The
    /// engine attaches every yielded pair under this rule's
    /// shared UUID.
    ///
    /// Iteration order is stable — `Predicated` yields once;
    /// `Table` yields in the entries' declared order.
    ///
    /// [`Predicate::LabelOneOf`]: super::predicate::Predicate::LabelOneOf
    pub fn attachments(&self) -> Box<dyn Iterator<Item = (Predicate, &ModalityRedactions)> + '_> {
        match self {
            Self::Predicated(r) => Box::new(std::iter::once((r.predicate.clone(), &r.action))),
            Self::Table(r) => Box::new(r.operators.iter().map(|entry| {
                (
                    Predicate::LabelOneOf {
                        labels: vec![entry.label.clone()],
                    },
                    &entry.action,
                )
            })),
        }
    }
}

/// Predicate-gated rule: one predicate, one action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredicatedRule {
    /// Stable identifier. UUIDv7 recommended.
    pub id: Uuid,
    /// Human-readable name. Display-only.
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

/// Per-label table rule: N labels, N actions, one shared identity.
///
/// Each entry compiles to a [`Rule::label`] attachment under this
/// rule's shared UUID / name / description — so the audit trail
/// records "rule X fired" without exposing the fan-out to the
/// reviewer. Meant for templates where a single policy intent
/// (e.g. "HIPAA Safe Harbor identifiers") routes different labels
/// to different operators.
///
/// [`Rule::label`]: elide_redaction::Rule::label
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TableRule {
    /// Stable identifier — shared by every entry the table
    /// expands into. UUIDv7 recommended.
    pub id: Uuid,
    /// Human-readable name. Display-only.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Per-label operator dispatch. Every entity whose label
    /// matches a listed [`LabelRef`] attaches the paired
    /// [`ModalityRedactions`]. Labels absent from the list are not
    /// affected by this rule and fall through to the next rule or
    /// the policy fallback.
    ///
    /// A [`Vec`] rather than a map keeps the author-supplied
    /// order — elide's anonymizer is first-match-wins, so wire
    /// order determines which entry fires when two match the
    /// same entity. Duplicate labels are the caller's bug; the
    /// engine attaches every entry, and the first one wins.
    pub operators: Vec<LabelEntry>,
}

/// One entry inside a [`TableRule`]: the label to match plus the
/// per-modality operators to run.
///
/// Kept as a named struct rather than a `(LabelRef, ModalityRedactions)`
/// tuple so the wire JSON reads `{"label": "email", "action": {…}}`
/// instead of a positional pair.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelEntry {
    /// Label the entry matches on.
    pub label: LabelRef,
    /// Per-modality operators to run for matching entities.
    pub action: ModalityRedactions,
}
