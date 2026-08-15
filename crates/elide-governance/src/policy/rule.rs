//! One rule inside a [`PolicyDefinition`]: identity + dispatch.
//!
//! Every rule carries the same identity metadata ([`id`], [`name`],
//! [`description`]) plus a [`RuleDispatch`] picking how targets are
//! selected:
//!
//! - [`RuleDispatch::Predicated`] — one predicate, one action.
//!   Fires when the predicate holds on the candidate entity. Fast
//!   paths in the engine compile [`Predicate::LabelOneOf`] with a
//!   single label to `Rule::label`, [`Predicate::TagOneOf`] with a
//!   single tag to `Rule::tag`, everything else to
//!   `Rule::predicate`.
//! - [`RuleDispatch::Table`] — N `(label, action)` entries under
//!   one shared identity. Every entry attaches as `Rule::label`
//!   directly and fires under the same rule id in the audit trail.
//!   Sugar over N predicated rules with identical id/name/description
//!   — keeps templates that fan out per-label operators (HIPAA
//!   Safe Harbor `age`→clamp, `date`→generalize, remainder→erase)
//!   from ballooning to one predicated rule per label with the
//!   same identity boilerplate repeated.
//!
//! [`PolicyDefinition`]: super::PolicyDefinition
//! [`Predicate`]: crate::Predicate
//! [`Predicate::LabelOneOf`]: crate::Predicate::LabelOneOf
//! [`Predicate::TagOneOf`]: crate::Predicate::TagOneOf
//! [`description`]: PolicyRule::description
//! [`id`]: PolicyRule::id
//! [`name`]: PolicyRule::name

use elide_core::entity::LabelRef;
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::predicate::Predicate;
use crate::redaction::ModalityRedactions;

/// One rule inside a [`PolicyDefinition`]. Identity is the UUID;
/// `name` / `description` are display-only. `dispatch` picks the
/// selection strategy.
///
/// [`PolicyDefinition`]: super::PolicyDefinition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    /// Stable identifier. UUIDv7 recommended. Engine stamps it
    /// into the redaction event's [`Attribution::description`] so
    /// reviewers can trace which rule fired. Every attachment a
    /// [`RuleDispatch::Table`] expands into shares this UUID.
    ///
    /// [`Attribution::description`]: elide_core::entity::audit::Attribution::description
    pub id: Uuid,
    /// Human-readable name. Display-only.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// How this rule selects targets and picks operators. See
    /// [`RuleDispatch`] for the two shipped strategies.
    #[serde(flatten)]
    pub dispatch: RuleDispatch,
}

impl PolicyRule {
    /// Flatten this rule into one or more `(predicate, action)`
    /// pairs. [`RuleDispatch::Predicated`] yields one pair
    /// verbatim; [`RuleDispatch::Table`] yields one pair per
    /// entry with a synthetic [`Predicate::LabelOneOf`] holding
    /// that single label. The engine attaches every yielded pair
    /// under this rule's shared UUID.
    ///
    /// Iteration order is stable — `Predicated` yields once;
    /// `Table` yields in the entries' declared order.
    ///
    /// [`Predicate::LabelOneOf`]: crate::Predicate::LabelOneOf
    pub fn attachments(&self) -> Box<dyn Iterator<Item = (Predicate, &ModalityRedactions)> + '_> {
        match &self.dispatch {
            RuleDispatch::Predicated { predicate, action } => {
                Box::new(std::iter::once((predicate.clone(), action.as_ref())))
            }
            RuleDispatch::Table { operators } => Box::new(operators.iter().map(|entry| {
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

/// How a [`PolicyRule`] selects candidate entities and pairs them
/// with operators.
///
/// Internally-tagged on the wire via `kind`:
/// `{ "kind": "predicated", "predicate": {...}, "action": {...} }`
/// or `{ "kind": "table", "operators": [...] }`. Every attachment
/// a `Table` expands into shares the parent rule's identity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuleDispatch {
    /// One predicate, one action. Fires when the predicate holds
    /// on the candidate entity.
    Predicated {
        /// Entity-level predicate that decides whether the rule
        /// fires on a given recognised entity. Composable; see
        /// [`Predicate`] for the full grammar.
        predicate: Predicate,
        /// Per-modality redaction operators applied when the
        /// predicate matches. Modalities the rule doesn't cover
        /// fall through to the policy fallback (or the next
        /// policy in the chain).
        ///
        /// Boxed to keep [`RuleDispatch`]'s stack footprint
        /// small — [`ModalityRedactions`] carries four optional
        /// per-modality operator enums and dominates the variant
        /// size. `Table`'s `Vec<LabelEntry>` already heap-allocates
        /// its entries, so boxing here keeps the two variants
        /// balanced without changing the wire form.
        action: Box<ModalityRedactions>,
    },
    /// N `(label, action)` entries under one shared identity.
    /// Every entity whose label matches a listed [`LabelRef`]
    /// attaches the paired [`ModalityRedactions`]. Labels absent
    /// from the list are not affected by this rule and fall
    /// through to the next rule or the policy fallback.
    ///
    /// A [`Vec`] rather than a map keeps the author-supplied
    /// order — elide's anonymizer is first-match-wins, so wire
    /// order determines which entry fires when two match the
    /// same entity. Duplicate labels are the caller's bug; the
    /// engine attaches every entry, and the first one wins.
    Table {
        /// Per-label operator dispatch.
        operators: Vec<LabelEntry>,
    },
}

/// One entry inside a [`RuleDispatch::Table`]: the label to match
/// plus the per-modality operators to run.
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
