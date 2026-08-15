//! SOC 2 secrets — erase credential material from evidence
//! artifacts before external sharing.
//!
//! Auditors ask for evidence (tickets, exports, dashboards,
//! deployment logs) that must not carry live secrets. The
//! shipped template is scoped to elide's credential-family
//! labels — `api_key`, `auth_token`, `private_key`, and
//! `crypto_address` — with a single [`Predicated`] rule that
//! erases every match.
//!
//! The framework fit is broad: this covers SOC 2 Trust Services
//! Criteria CC6.1 (logical access controls) and CC6.7
//! (transmission / disposal of confidential information), and
//! satisfies ISO 27001 A.9.2.4 (secret authentication
//! information management) since the label set is identical.
//! Cryptocurrency wallet addresses are included as
//! credential-adjacent — in a shipped log, a wallet address is a
//! live pointer at production assets under the org's control.
//!
//! Unlike HIPAA §164.514, SOC 2's Common Criteria do not
//! enumerate a fixed identifier list; the label scope here
//! reflects industry-standard secrets-scanning practice rather
//! than framework text. Callers whose evidence workflow needs
//! richer coverage (session identifiers, JWT tokens embedded in
//! log lines) extend the returned [`PolicyDefinition`]'s labels
//! and group before submission.
//!
//! [`PolicyDefinition`]: elide_governance::PolicyDefinition
//! [`Predicated`]: elide_governance::RuleDispatch::Predicated

use elide_core::entity::LabelRef;
use elide_governance::predicate::Predicate;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use jiff::civil::Date;
use semver::Version;
use uuid::{Uuid, uuid};

use super::Template;

/// Group name every SOC 2 secrets rule references.
pub(crate) const GROUP_NAME: &str = "soc2_secrets";

/// Elide-builtin labels the group covers. Every entry is
/// credential material or a credential-adjacent identifier a
/// leak would treat as sensitive:
///
/// - `api_key` — service / provider API keys (AWS, Stripe,
///   GitHub, generic).
/// - `auth_token` — bearer tokens, session tokens, refresh
///   tokens.
/// - `private_key` — PEM-encoded keys and OpenSSH private
///   material.
/// - `crypto_address` — wallet receive/send addresses (BTC, ETH,
///   …). Not authentication material per se, but a leak points
///   at production assets under the org's control.
const SOC2_SECRETS_LABELS: &[LabelRef] = &[
    LabelRef::from_static("api_key"),
    LabelRef::from_static("auth_token"),
    LabelRef::from_static("private_key"),
    LabelRef::from_static("crypto_address"),
];

const POLICY_ID: Uuid = uuid!("019a1234-0000-7000-8000-000000000001");
const RULE_ID: Uuid = uuid!("019a1234-0000-7000-8000-000000000002");

/// Build the SOC 2 secrets template.
pub(crate) fn template() -> Template {
    Template {
        id: "soc2_secrets".into(),
        name: "SOC 2 secrets scan".into(),
        version: Version::new(1, 0, 0),
        // SOC 2 Trust Services Criteria (2017 revision, effective
        // Dec 15, 2018) is the current TSC edition.
        effective_date: Date::constant(2018, 12, 15),
        description: Some(
            "SOC 2 CC6.1 / CC6.7 — erase API keys, auth tokens, private keys, and \
             wallet addresses from evidence artifacts before external sharing. \
             Also satisfies ISO 27001 A.9.2.4."
                .into(),
        ),
        policy: policy(),
    }
}

fn group() -> LabelGroup {
    LabelGroup {
        name: GROUP_NAME.into(),
        description: Some(
            "Credential material and credential-adjacent identifiers whose leak \
             into an evidence artifact violates SOC 2 CC6.1 (logical access) or \
             CC6.7 (transmission / disposal). API keys, auth tokens, private \
             keys, and wallet addresses."
                .into(),
        ),
        labels: SOC2_SECRETS_LABELS.to_vec(),
    }
}

fn policy() -> PolicyDefinition {
    PolicyDefinition {
        id: POLICY_ID,
        name: "soc2-secrets-scan".into(),
        description: Some(
            "Erase every secret before an evidence artifact leaves the org. Live \
             credentials in shipped tickets / logs / exports are a control failure \
             under SOC 2 CC6.x and ISO 27001 A.9.2.4."
                .into(),
        ),
        labels: Labels {
            builtins: SOC2_SECRETS_LABELS.to_vec(),
            custom: Vec::new(),
        },
        groups: vec![group()],
        rules: vec![erase_rule()],
        fallback: None,
    }
}

fn erase_rule() -> PolicyRule {
    PolicyRule {
        id: RULE_ID,
        name: "soc2-secrets-erase".into(),
        description: Some("Erase any entity whose label falls in the soc2_secrets group.".into()),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: GROUP_NAME.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_covers_every_credential_label() {
        for want in ["api_key", "auth_token", "private_key", "crypto_address"] {
            assert!(
                SOC2_SECRETS_LABELS.iter().any(|l| l.as_str() == want),
                "SOC2_SECRETS_LABELS must include `{want}`",
            );
        }
    }

    #[test]
    fn scope_excludes_personal_data_labels() {
        // The template is credential-focused; personal-data
        // labels belong in HIPAA/GDPR/CCPA/PII-baseline scopes,
        // not here. A future edit that folds PII in silently
        // trips this.
        for outside in [
            "person_name",
            "email_address",
            "phone_number",
            "government_id",
            "date_of_birth",
        ] {
            assert!(
                !SOC2_SECRETS_LABELS.iter().any(|l| l.as_str() == outside),
                "SOC2_SECRETS_LABELS must NOT include personal-data label `{outside}`",
            );
        }
    }

    #[test]
    fn erase_rule_targets_the_group() {
        let policy = &template().policy;
        assert_eq!(policy.rules.len(), 1, "single bulk-erase rule");
        let RuleDispatch::Predicated { predicate, action } = &policy.rules[0].dispatch else {
            panic!("rule must be Predicated dispatch");
        };
        let Predicate::LabelInGroup { group } = predicate else {
            panic!("predicate must match by label group");
        };
        assert_eq!(group, GROUP_NAME);
        assert!(matches!(action.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let a = template();
        let b = template();
        assert_eq!(a.id, b.id);
        assert_eq!(a.policy.id, b.policy.id);
        assert_eq!(a.policy.rules[0].id, b.policy.rules[0].id);
    }
}
