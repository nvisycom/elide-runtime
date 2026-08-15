//! [`LabelScope`]: a named, attributed set of labels a policy
//! detects.
//!
//! A policy's scopes are its recognition vocabulary. The union of
//! every scope (plus the policy's inline `custom` schemas) is what
//! the engine tells recognizers to hunt for, so a label no scope
//! names is never detected and no rule can fire on it.
//!
//! Scopes are named so a rule can target one directly via
//! [`Predicate::LabelInScope`], and attributed so an audit records
//! *why* the set exists (HIPAA's eighteen identifiers, GDPR
//! Article 9(1)'s nine special categories) rather than only which
//! rule fired.
//!
//! Detecting more than the rules act on is the point, not a
//! mistake: a policy scopes a whole regulatory category, writes
//! rules for the labels needing special treatment, and lets
//! [`fallback`] sweep the rest.
//!
//! [`PolicyDefinition`]: super::PolicyDefinition
//! [`Predicate::LabelInScope`]: crate::Predicate::LabelInScope
//! [`fallback`]: super::PolicyDefinition::fallback

use elide_core::entity::LabelRef;
use elide_core::entity::audit::AttributionKind;
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A named set of labels a [`PolicyDefinition`] detects.
///
/// Scopes live on the policy that declares them and are visible
/// only to that policy's own rules. Two policies that both declare
/// `hipaa_18` with different labelsets stay independent: nothing is
/// stamped onto the shared label catalog, so there is no request-
/// wide namespace one policy could use to reach into another's.
///
/// Templates ship one scope per canonical label list
/// (`"hipaa_safe_harbor"`, `"gdpr_article_9"`, `"ccpa_personal_information"`).
/// A policy may declare several: the union is what it detects, and
/// a rule can target one by name.
///
/// **Unknown scope names error at request validation**, not at
/// apply time. A typo doesn't silently underfire.
///
/// [`PolicyDefinition`]: super::PolicyDefinition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelScope {
    /// Stable name a [`Predicate::LabelInScope`] references.
    ///
    /// Free-form; a policy layer picks the vocabulary. Recommend
    /// snake_case identifiers (`hipaa_safe_harbor`,
    /// `gdpr_article_9`): they read cleanly in audit provenance.
    ///
    /// [`Predicate::LabelInScope`]: crate::Predicate::LabelInScope
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// Why this set exists: the authority that defines it.
    ///
    /// A scope usually maps to one regulatory category (HIPAA's
    /// eighteen identifiers, GDPR Article 9(1)'s nine special
    /// categories), so this is where that mapping is recorded as
    /// data rather than prose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<AttributionKind>,
    /// The labels this scope covers, by ref.
    ///
    /// A label the current build doesn't emit (a modality-gated
    /// one, say) is simply never detected; rules keyed off the
    /// scope still fire on whatever labels *are* present.
    pub labels: Vec<LabelRef>,
}

impl LabelScope {
    /// A scope named `name` covering `labels`, with no description
    /// or attribution.
    pub fn new(
        name: impl Into<HipStr<'static>>,
        labels: impl IntoIterator<Item = LabelRef>,
    ) -> Self {
        Self {
            name: name.into(),
            description: None,
            attribution: None,
            labels: labels.into_iter().collect(),
        }
    }

    /// Attach the authority this scope answers to.
    #[must_use]
    pub fn with_attribution(mut self, attribution: AttributionKind) -> Self {
        self.attribution = Some(attribution);
        self
    }

    /// Attach a reviewer-facing description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<HipStr<'static>>) -> Self {
        self.description = Some(description.into());
        self
    }
}
