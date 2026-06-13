//! Policy types: authored vocabulary for redaction governance.
//!
//! A [`Policy<M>`] is a named, versioned governance artefact: an
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

mod condition;
pub mod redaction;
mod retention;
mod rule;
mod selector;

use derive_builder::Builder;
use derive_more::{From, IsVariant};
use hipstr::HipStr;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::condition::Condition;
pub use self::redaction::AnyRedaction;
pub use self::retention::{Retention, RetentionPolicy, RetentionScope};
pub use self::rule::{Action, PolicyRule};
pub use self::selector::EntitySelector;
use crate::modality::{Audio, DocumentModality, Image, Tabular, Text};

/// A named, versioned governance policy for one modality.
///
/// Identified by [`name`] + [`version`]; the name must be unique
/// within a single [`DetectionInput::policies`] submission. Held as
/// a [`HipStr<'static>`] so per-decision audit stamps and per-run
/// snapshots share refcounts rather than allocating.
///
/// [`name`]: Self::name
/// [`version`]: Self::version
/// [`DetectionInput::policies`]: crate::pipeline::detection::DetectionInput::policies
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "PolicyBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Policy<M: DocumentModality> {
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
    /// Ordered list of rules. First matching rule wins.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule<M>>,
    /// Fallback action for entities that no [`PolicyRule`] in this
    /// policy matched. Consulted only after every rule in this
    /// policy has been considered; the engine then moves to the next
    /// policy in the per-run chain. `None` means "this policy has no
    /// opinion for unmatched entities; let the next policy decide."
    ///
    /// Authored on its own table — e.g. `[defaultAction]
    /// action = "redact"` — so the same TOML shape works as for
    /// rules.
    #[builder(default, setter(into = false))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_action: Option<Action<M>>,
    /// Data retention lifecycle rules.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention: Vec<RetentionPolicy>,
}

impl<M: DocumentModality> Policy<M> {
    /// Start building a new policy.
    pub fn builder() -> PolicyBuilder<M> {
        PolicyBuilder::default()
    }
}

/// A modality-erased [`Policy`].
///
/// [`DetectionInput::policies`] is `Vec<AnyPolicy>` so a single
/// detect call can submit policies covering every modality the
/// content will fan out into (e.g. a PDF that produces both `Text`
/// and `Image` envelopes can carry one [`Policy<Text>`] and one
/// [`Policy<Image>`] in the same submission).
///
/// Wire format mirrors [`AnyAudit`]: tagged by `modality`, with the
/// inner policy's fields flattened into the same JSON object.
///
/// ```json
/// { "modality": "text", "id": "...", "name": "...", "rules": [...] }
/// ```
///
/// [`DetectionInput::policies`]: crate::pipeline::detection::DetectionInput::policies
/// [`AnyAudit`]: crate::document::provenance::AnyAudit
#[derive(Debug, Clone, From, IsVariant, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum AnyPolicy {
    /// Text-modality policy.
    Text(Policy<Text>),
    /// Tabular-modality policy.
    Tabular(Policy<Tabular>),
    /// Image-modality policy.
    Image(Policy<Image>),
    /// Audio-modality policy.
    Audio(Policy<Audio>),
}

impl AnyPolicy {
    /// Author-supplied name of the contained policy. Echoed
    /// verbatim into the audit's [`PolicyDecisionRef`] every time a
    /// rule in this policy fires. Returned as `&str` for ergonomic
    /// comparison; callers needing a refcount-cheap owned clone
    /// can match the variant directly and clone the inner
    /// [`HipStr`].
    pub fn name(&self) -> &str {
        match self {
            Self::Text(p) => &p.name,
            Self::Tabular(p) => &p.name,
            Self::Image(p) => &p.name,
            Self::Audio(p) => &p.name,
        }
    }

    /// Header-card summary suitable for storing on the detection
    /// record without inlining the full rule body.
    pub fn digest(&self) -> PolicyDigest {
        match self {
            Self::Text(p) => PolicyDigest::from_policy(p),
            Self::Tabular(p) => PolicyDigest::from_policy(p),
            Self::Image(p) => PolicyDigest::from_policy(p),
            Self::Audio(p) => PolicyDigest::from_policy(p),
        }
    }
}

/// Header card identifying a policy submitted to a run, persisted on
/// the detection record so a reader can render
/// `"<name> v<version>"` without the caller having to keep the
/// original [`AnyPolicy`] bytes around.
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

impl PolicyDigest {
    /// Distil a digest from a typed policy.
    pub fn from_policy<M: DocumentModality>(policy: &Policy<M>) -> Self {
        Self {
            name: policy.name.clone(),
            version: policy.version.clone(),
        }
    }
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
    pub fn new(
        policy_name: HipStr<'static>,
        rule_name: Option<HipStr<'static>>,
    ) -> Self {
        Self {
            policy_name,
            rule_name,
        }
    }
}

/// Validate the name uniqueness invariants the audit's
/// [`PolicyDecisionRef`] depends on:
///
/// - No two policies in the submission share a [`Policy::name`].
/// - No two rules inside the same policy share a [`PolicyRule::name`].
///
/// Returns a validation error naming the offending duplicate. Called
/// at the top of [`Engine::detect`] so authors learn about a broken
/// namespace before any work runs.
///
/// [`Engine::detect`]: crate::pipeline::Engine::detect
pub fn validate_policy_namespace(policies: &[AnyPolicy]) -> Result<(), nvisy_core::Error> {
    use std::collections::HashSet;

    const TARGET: &str = "nvisy_engine::policy::validate";

    fn check_rule_names<'a, I: ExactSizeIterator<Item = &'a str>>(
        policy_name: &str,
        rule_names: I,
    ) -> Result<(), nvisy_core::Error> {
        let mut seen_rules: HashSet<&str> = HashSet::with_capacity(rule_names.len());
        for rule_name in rule_names {
            if !seen_rules.insert(rule_name) {
                return Err(nvisy_core::Error::validation(
                    format!(
                        "policy `{policy_name}` has duplicate rule name `{rule_name}`; \
                         rule names must be unique within their owning policy",
                    ),
                    TARGET,
                ));
            }
        }
        Ok(())
    }

    let mut seen_policies: HashSet<&str> = HashSet::with_capacity(policies.len());
    for any in policies {
        let policy_name = any.name();
        if !seen_policies.insert(policy_name) {
            return Err(nvisy_core::Error::validation(
                format!(
                    "duplicate policy name `{policy_name}` in detection submission; \
                     audit references rules by policy + rule name, so policy \
                     names must be unique within a single detect call",
                ),
                TARGET,
            ));
        }

        match any {
            AnyPolicy::Text(p) => {
                check_rule_names(policy_name, p.rules.iter().map(|r| r.name.as_str()))?;
            }
            AnyPolicy::Tabular(p) => {
                check_rule_names(policy_name, p.rules.iter().map(|r| r.name.as_str()))?;
            }
            AnyPolicy::Image(p) => {
                check_rule_names(policy_name, p.rules.iter().map(|r| r.name.as_str()))?;
            }
            AnyPolicy::Audio(p) => {
                check_rule_names(policy_name, p.rules.iter().map(|r| r.name.as_str()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modality::Text;

    fn text_policy(name: &str, rules: &[&str]) -> AnyPolicy {
        let rules = rules
            .iter()
            .map(|n| PolicyRule {
                name: HipStr::from(*n),
                selector: EntitySelector::default(),
                action: Action::Suppress,
                conditions: Vec::new(),
                enabled: true,
            })
            .collect();
        AnyPolicy::Text(Policy::<Text> {
            name: HipStr::from(name),
            version: Version::new(1, 0, 0),
            description: None,
            rules,
            default_action: None,
            retention: Vec::new(),
        })
    }

    #[test]
    fn empty_namespace_passes() {
        assert!(validate_policy_namespace(&[]).is_ok());
    }

    #[test]
    fn distinct_policies_with_distinct_rules_pass() {
        let policies = vec![
            text_policy("gdpr", &["redact-ssn", "redact-email"]),
            text_policy("hipaa", &["redact-mrn"]),
        ];
        assert!(validate_policy_namespace(&policies).is_ok());
    }

    #[test]
    fn duplicate_policy_name_fails() {
        let policies = vec![text_policy("gdpr", &["a"]), text_policy("gdpr", &["b"])];
        let err = validate_policy_namespace(&policies).unwrap_err();
        assert!(err.to_string().contains("duplicate policy name `gdpr`"));
    }

    #[test]
    fn duplicate_rule_name_fails() {
        let policies = vec![text_policy("gdpr", &["dup", "dup"])];
        let err = validate_policy_namespace(&policies).unwrap_err();
        assert!(
            err.to_string()
                .contains("duplicate rule name `dup`"),
            "got: {err}"
        );
    }
}
