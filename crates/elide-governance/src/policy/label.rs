//! Per-policy label catalog vocabulary and per-request named
//! label groups.
//!
//! Two shapes:
//!
//! - [`Labels`] — carried on each [`PolicyDefinition`]. Selects
//!   builtins from elide-core's shipped set and adds inline
//!   custom schemas. The engine unions every submitted policy's
//!   `labels` into one `elide_core::entity::LabelCatalog` at
//!   request-compile time.
//! - [`LabelGroup`] — a named cluster of [`LabelRef`]s carried
//!   on the policy that declares it. Templates ship groups
//!   (`"hipaa_18"`, `"gdpr_article_9"`, `"pci_chd"`); rules
//!   inside the same policy reference groups by name via
//!   [`Predicate::LabelInGroup`]. Membership resolves when the
//!   predicate is evaluated, against the group table the policy
//!   carries — the catalog stays untouched.
//!
//! [`PolicyDefinition`]: super::PolicyDefinition
//! [`Predicate::LabelInGroup`]: crate::Predicate::LabelInGroup

use elide_core::entity::{Label, LabelRef};
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-policy label-catalog selection.
///
/// Picks builtins by name + adds inline custom schemas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Labels {
    /// Builtin label names to enable.
    ///
    /// E.g. `"email_address"`, `"phone_number"`. An unknown name
    /// is rejected at request compile time — a typo fails loudly
    /// rather than quietly dropping the label from the policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builtins: Vec<LabelRef>,
    /// Custom labels defined inline by the caller.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<Label>,
}

impl Labels {
    /// `true` when neither source contributes any label.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.builtins.is_empty() && self.custom.is_empty()
    }
}

/// Named cluster of [`LabelRef`]s a policy's rules can reference
/// by name via [`Predicate::LabelInGroup`].
///
/// Groups live on the [`PolicyDefinition`] that declares them
/// and are visible only to that policy's own rules. Templates
/// ship one group per canonical label list (`"hipaa_18"`,
/// `"gdpr_article_9"`, `"pci_chd"`, `"pci_sad"`), and every
/// rule that targets that list references the group by name
/// instead of respelling the labels. When elide adds a new label
/// to a category, extending the group covers every rule that
/// referenced it — no rule edit.
///
/// **Compilation**: groups stay on the policy. Evaluating a
/// [`Predicate::LabelInGroup { group }`] looks the name up in
/// the policy's own group table and tests the entity's label for
/// membership. Nothing is stamped onto the label catalog, so two
/// policies that both declare `hipaa_18` with different labelsets
/// cannot step on each other.
///
/// **Unknown group names error at request validation**, not at
/// apply time. A typo doesn't silently underfire.
///
/// [`PolicyDefinition`]: super::PolicyDefinition
/// [`Predicate::LabelInGroup`]: crate::Predicate::LabelInGroup
/// [`Predicate::LabelInGroup { group }`]: crate::Predicate::LabelInGroup
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelGroup {
    /// Stable name a [`Predicate::LabelInGroup`] references.
    ///
    /// Free-form; a policy layer picks the vocabulary. Recommend
    /// snake_case identifiers (`hipaa_18`, `gdpr_article_9`) —
    /// they read cleanly in audit provenance.
    ///
    /// [`Predicate::LabelInGroup`]: crate::Predicate::LabelInGroup
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// Labels this group covers, by ref.
    ///
    /// A label that doesn't appear in the request's compiled
    /// [`LabelCatalog`] is silently skipped at tag-synthesis time
    /// — a group can safely list labels the current build
    /// doesn't emit (e.g. modality-gated ones); rules keyed off
    /// the group still fire on whatever labels *are* present.
    ///
    /// [`LabelCatalog`]: elide_core::entity::LabelCatalog
    pub labels: Vec<LabelRef>,
}
