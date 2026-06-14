//! Policy types: authored vocabulary for redaction governance.
//!
//! A [`Policy`] is a named, versioned governance artefact: an
//! ordered list of [`PolicyRule`]s plus an optional fallback
//! [`Policy::default_action`] plus a retention configuration.
//! Policies are reusable — the same policy can participate in many
//! runs.
//!
//! Per-run composition (which policies apply to *this* run, in what
//! order) lives in the engine; the ontology does not model it.
//! Precedence is positional: in a run, the first policy in the
//! caller-supplied list is highest precedence; within a policy, the
//! first matching rule wins; the policy's `default_action` fires
//! only when no rule in that policy matched.

mod audit;
mod condition;
pub mod redaction;
mod retention;
mod rule;
mod selector;
mod suppress;

use derive_builder::Builder;
use hipstr::HipStr;
use nvisy_core::entity::EntityLabel;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::audit::AuditAction;
pub use self::condition::Condition;
pub use self::redaction::AnyRedaction;
pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::rule::{Action, PolicyRule};
pub use self::selector::EntitySelector;
pub use self::suppress::SuppressAction;

/// A named, versioned governance policy.
///
/// Identified by [`name`] + [`version`]; the name must be unique
/// within a single [`DetectionInput::policies`] submission. Held as
/// a [`HipStr<'static>`] so per-decision audit stamps and per-run
/// snapshots share refcounts rather than allocating.
///
/// Modality is not part of the type. Each rule's action carries
/// per-modality operator specs ([`ModalityRedactions`]); the apply
/// phase picks the operator matching the entity's modality.
///
/// [`name`]: Self::name
/// [`version`]: Self::version
/// [`DetectionInput::policies`]: crate::detection::DetectionInput::policies
/// [`ModalityRedactions`]: crate::policy::redaction::ModalityRedactions
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// Author-supplied policy name. Must be unique across every
    /// policy in a detection submission; audit entries reference
    /// this string verbatim.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
    /// Description of the policy's purpose.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Entity labels this policy operates over. Every label name a
    /// [`PolicyRule::selector`] references must appear here. The
    /// engine unions every submitted policy's `labels` into a
    /// per-request [`EntityLabelCatalog`] used to drive recognizer
    /// dispatch and tag-based selector matching. Two policies
    /// declaring the same label name with different
    /// `(description, tags)` are a conflict and fail the request.
    ///
    /// [`EntityLabelCatalog`]: nvisy_core::entity::EntityLabelCatalog
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<EntityLabel>,
    /// Ordered list of rules. First matching rule wins.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Fallback action for entities that no [`PolicyRule`] in this
    /// policy matched. Consulted only after every rule in this
    /// policy has been considered; the engine then moves to the next
    /// policy in the per-run chain. `None` means "this policy has no
    /// opinion for unmatched entities; let the next policy decide."
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_action: Option<Action>,
    /// Data retention lifecycle rules.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}

impl Policy {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder {
        PolicyBuilder::default()
    }

    /// Header-card summary suitable for storing on the detection
    /// record without inlining the full rule body.
    pub fn digest(&self) -> PolicyDigest {
        PolicyDigest {
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }
}

/// Header card identifying a policy submitted to a run, persisted on
/// the detection record so a reader can render
/// `"<name> v<version>"` without the caller having to keep the
/// original [`Policy`] bytes around.
///
/// Carries name + version only — no rules. The rule body lives only
/// in the caller's submission; the engine remembers which policies
/// ran by digest, and stamps every audit decision with a
/// [`PolicyDecisionRef`] pointing at one of them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDigest {
    /// Policy name; matches `Policy::name` of the submitted policy.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Policy version.
    #[schemars(with = "String")]
    pub version: Version,
}

/// Reference to the specific rule (or fallback) that produced a
/// decision, stamped onto every policy-driven [`AuditEntry`].
///
/// Names map back into the [`Policy`] / [`PolicyRule`] structures
/// the caller submitted. `rule_name` is `None` when the producing
/// rule was the policy's [`default_action`] fallback rather than a
/// concrete named rule.
///
/// Both fields hold [`HipStr<'static>`] clones so audit-heavy passes
/// share refcounts rather than allocating per-entity.
///
/// [`AuditEntry`]: crate::document::provenance::AuditEntry
/// [`default_action`]: Policy::default_action
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDecisionRef {
    /// Name of the policy that made the decision. Matches the
    /// `name` of one [`PolicyDigest`] on the detection record.
    #[schemars(with = "String")]
    pub policy_name: HipStr<'static>,
    /// Name of the producing rule inside its policy. `None` means
    /// the policy's [`Policy::default_action`] fallback fired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub rule_name: Option<HipStr<'static>>,
}

impl PolicyDecisionRef {
    /// Construct a reference from a policy + rule name.
    pub fn new(policy_name: HipStr<'static>, rule_name: Option<HipStr<'static>>) -> Self {
        Self {
            policy_name,
            rule_name,
        }
    }
}
