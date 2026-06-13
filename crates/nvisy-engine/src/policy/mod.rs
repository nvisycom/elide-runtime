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
use nvisy_core::entity::{EntityLabel, EntityLabelCatalog};
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
    /// Entity labels this policy operates over. Every label name a
    /// [`PolicyRule::selector`] references must appear here. The
    /// engine unions every submitted policy's `labels` into a
    /// per-request [`EntityLabelCatalog`] used to drive recognizer
    /// dispatch and tag-based selector matching. Two policies
    /// declaring the same label name with different
    /// `(description, tags)` are a conflict and fail the request.
    #[builder(default = "Vec::new()")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<EntityLabel>,
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

    /// Entity labels declared on the contained policy.
    pub fn labels(&self) -> &[EntityLabel] {
        match self {
            Self::Text(p) => &p.labels,
            Self::Tabular(p) => &p.labels,
            Self::Image(p) => &p.labels,
            Self::Audio(p) => &p.labels,
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
    pub fn new(policy_name: HipStr<'static>, rule_name: Option<HipStr<'static>>) -> Self {
        Self {
            policy_name,
            rule_name,
        }
    }
}

/// Union every submitted policy's [`Policy::labels`] into a single
/// [`EntityLabelCatalog`] driving recognizer dispatch and selector
/// tag matching for one request.
///
/// Two policies declaring the same label name with non-equal
/// `(description, tags)` are a conflict and fail the request — the
/// engine cannot pick a winner without violating one author's
/// intent. Identical re-declarations of the same label are allowed
/// (idempotent).
///
/// # Errors
///
/// Returns a validation error naming both policies on the first
/// conflict encountered.
pub fn unify_labels(policies: &[AnyPolicy]) -> Result<EntityLabelCatalog, nvisy_core::Error> {
    use std::collections::HashMap;

    const TARGET: &str = "nvisy_engine::policy::unify_labels";

    let mut catalog = EntityLabelCatalog::new();
    let mut origin: HashMap<HipStr<'static>, HipStr<'static>> = HashMap::new();

    for any in policies {
        let policy_name = HipStr::from(any.name().to_owned());
        for label in any.labels() {
            if let Some(existing) = catalog.lookup(label.name.as_str()) {
                if existing != label {
                    let prior = origin
                        .get(&label.name)
                        .map(HipStr::as_str)
                        .unwrap_or("<unknown>");
                    return Err(nvisy_core::Error::validation(
                        format!(
                            "policy `{}` redeclares label `{}` with a different body \
                             than policy `{prior}`; resolve the divergence so both \
                             policies share one definition",
                            policy_name, label.name,
                        ),
                        TARGET,
                    ));
                }
                continue;
            }
            origin.insert(label.name.clone(), policy_name.clone());
            catalog.insert(label.clone());
        }
    }
    Ok(catalog)
}

/// Validate every [`EntitySelector::labels`] entry across every
/// rule references a label name registered in `catalog`. Selectors
/// targeting unregistered names would never match — silently
/// dropping them would let a typo bypass redaction without anyone
/// noticing.
///
/// # Errors
///
/// Returns a validation error naming the policy, rule, and missing
/// label on the first violation encountered.
pub fn validate_selector_labels(
    policies: &[AnyPolicy],
    catalog: &EntityLabelCatalog,
) -> Result<(), nvisy_core::Error> {
    const TARGET: &str = "nvisy_engine::policy::validate_selector_labels";

    fn check<M: DocumentModality>(
        policy_name: &str,
        rules: &[PolicyRule<M>],
        catalog: &EntityLabelCatalog,
    ) -> Result<(), nvisy_core::Error> {
        for rule in rules {
            for label in &rule.selector.labels {
                if catalog.lookup(label.as_str()).is_none() {
                    return Err(nvisy_core::Error::validation(
                        format!(
                            "policy `{policy_name}` rule `{}` selects label `{}` \
                             which no policy declares; declare it on `policy.labels` \
                             or remove the selector entry",
                            rule.name,
                            label.as_str(),
                        ),
                        TARGET,
                    ));
                }
            }
        }
        Ok(())
    }

    for any in policies {
        let policy_name = any.name();
        match any {
            AnyPolicy::Text(p) => check(policy_name, &p.rules, catalog)?,
            AnyPolicy::Tabular(p) => check(policy_name, &p.rules, catalog)?,
            AnyPolicy::Image(p) => check(policy_name, &p.rules, catalog)?,
            AnyPolicy::Audio(p) => check(policy_name, &p.rules, catalog)?,
        }
    }
    Ok(())
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
        text_policy_with(name, rules, Vec::new())
    }

    fn text_policy_with(name: &str, rules: &[&str], labels: Vec<EntityLabel>) -> AnyPolicy {
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
            labels,
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
            err.to_string().contains("duplicate rule name `dup`"),
            "got: {err}"
        );
    }

    fn label(name: &str, tags: &[&'static str]) -> EntityLabel {
        EntityLabel::new(name.to_owned()).with_tags(tags.iter().copied())
    }

    #[test]
    fn unify_labels_empty_input_is_empty_catalog() {
        let cat = unify_labels(&[]).unwrap();
        assert!(cat.is_empty());
    }

    #[test]
    fn unify_labels_unions_disjoint_policies() {
        let policies = vec![
            text_policy_with("gdpr", &[], vec![label("email_address", &["pii"])]),
            text_policy_with("hipaa", &[], vec![label("diagnosis", &["phi"])]),
        ];
        let cat = unify_labels(&policies).unwrap();
        assert_eq!(cat.len(), 2);
        assert!(cat.lookup("email_address").is_some());
        assert!(cat.lookup("diagnosis").is_some());
    }

    #[test]
    fn unify_labels_idempotent_redeclaration_passes() {
        let lbl = label("email_address", &["pii"]);
        let policies = vec![
            text_policy_with("gdpr", &[], vec![lbl.clone()]),
            text_policy_with("ccpa", &[], vec![lbl]),
        ];
        let cat = unify_labels(&policies).unwrap();
        assert_eq!(cat.len(), 1);
    }

    fn text_policy_rule(name: &str, selector_labels: &[&str]) -> AnyPolicy {
        let labels: Vec<nvisy_core::entity::EntityLabelRef> = selector_labels
            .iter()
            .map(|s| nvisy_core::entity::EntityLabelRef::new(HipStr::from(s.to_owned())))
            .collect();
        AnyPolicy::Text(Policy::<Text> {
            name: HipStr::from(name),
            version: Version::new(1, 0, 0),
            description: None,
            labels: Vec::new(),
            rules: vec![PolicyRule {
                name: HipStr::from("r"),
                selector: EntitySelector {
                    labels,
                    ..EntitySelector::default()
                },
                action: Action::Suppress,
                conditions: Vec::new(),
                enabled: true,
            }],
            default_action: None,
            retention: Vec::new(),
        })
    }

    #[test]
    fn validate_selector_labels_passes_when_label_in_catalog() {
        let catalog = EntityLabelCatalog::new().with_label(label("email_address", &["pii"]));
        let policies = vec![text_policy_rule("gdpr", &["email_address"])];
        assert!(validate_selector_labels(&policies, &catalog).is_ok());
    }

    #[test]
    fn validate_selector_labels_fails_on_missing_label() {
        let catalog = EntityLabelCatalog::new();
        let policies = vec![text_policy_rule("gdpr", &["email_address"])];
        let err = validate_selector_labels(&policies, &catalog).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("email_address"), "got: {msg}");
        assert!(msg.contains("gdpr"), "got: {msg}");
    }

    #[test]
    fn unify_labels_conflicting_redeclaration_fails() {
        let policies = vec![
            text_policy_with("gdpr", &[], vec![label("email_address", &["pii"])]),
            text_policy_with(
                "ccpa",
                &[],
                vec![label("email_address", &["pii", "contact_info"])],
            ),
        ];
        let err = unify_labels(&policies).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ccpa"), "got: {msg}");
        assert!(msg.contains("gdpr"), "got: {msg}");
        assert!(msg.contains("email_address"), "got: {msg}");
    }
}
