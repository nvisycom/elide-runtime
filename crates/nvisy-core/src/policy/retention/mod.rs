//! Data retention policy types + cross-policy resolution.

mod duration;

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

pub use self::duration::Retention;
use super::Policy;

/// What class of data a retention policy applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[derive(EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum RetentionScope {
    /// Original ingested content before redaction.
    OriginalContent,
    /// Redacted output artifacts.
    RedactedOutput,
    /// Audit log entries.
    AuditLogs,
}

/// A single retention rule: scope + duration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RetentionPolicy {
    /// What class of data this applies to.
    pub scope: RetentionScope,
    /// How long to retain data.
    pub retention: Retention,
}

/// Resolve the effective per-scope retention across `policies`.
///
/// **Strictest wins**: when two policies both name a retention
/// for the same scope, the stricter [`Retention`] is kept
/// (smaller `days`, with `ZeroRetention` strictest and
/// `Indefinite` laxest — see [`Retention`]'s `Ord` impl).
///
/// Scopes no policy mentions are absent from the returned map;
/// the caller chooses how to treat that (deployment default,
/// indefinite, …) — that's not the resolver's concern.
///
/// Order of `policies` does not affect the result. Empty input
/// yields an empty map.
pub fn resolve_retention<'a>(
    policies: impl IntoIterator<Item = &'a Policy>,
) -> HashMap<RetentionScope, Retention> {
    let mut out = HashMap::new();
    for policy in policies {
        for rule in &policy.retention {
            let entry = out.entry(rule.scope).or_insert(rule.retention);
            if rule.retention < *entry {
                *entry = rule.retention;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use hipstr::HipStr;
    use semver::Version;
    use uuid::Uuid;

    use super::*;

    fn policy_with(rules: Vec<RetentionPolicy>) -> Policy {
        Policy {
            id: Uuid::now_v7(),
            name: HipStr::from("test"),
            version: Version::new(1, 0, 0),
            description: None,
            applies_when: None,
            labels: Vec::new(),
            rules: Vec::new(),
            fallback: None,
            retention: rules,
        }
    }

    fn rule(scope: RetentionScope, retention: Retention) -> RetentionPolicy {
        RetentionPolicy { scope, retention }
    }

    #[test]
    fn empty_input_yields_empty_map() {
        let empty: [&Policy; 0] = [];
        let resolved = resolve_retention(empty);
        assert!(resolved.is_empty());
    }

    #[test]
    fn single_policy_passes_through() {
        let p = policy_with(vec![
            rule(RetentionScope::OriginalContent, Retention::ZeroRetention),
            rule(
                RetentionScope::RedactedOutput,
                Retention::Duration { days: 30 },
            ),
        ]);
        let resolved = resolve_retention([&p]);
        assert_eq!(resolved.len(), 2);
        assert_eq!(
            resolved[&RetentionScope::OriginalContent],
            Retention::ZeroRetention,
        );
        assert_eq!(
            resolved[&RetentionScope::RedactedOutput],
            Retention::Duration { days: 30 },
        );
    }

    #[test]
    fn unmentioned_scope_is_absent() {
        let p = policy_with(vec![rule(
            RetentionScope::OriginalContent,
            Retention::ZeroRetention,
        )]);
        let resolved = resolve_retention([&p]);
        assert!(!resolved.contains_key(&RetentionScope::RedactedOutput));
        assert!(!resolved.contains_key(&RetentionScope::AuditLogs));
    }

    #[test]
    fn zero_beats_duration_and_indefinite() {
        let strict = policy_with(vec![rule(
            RetentionScope::OriginalContent,
            Retention::ZeroRetention,
        )]);
        let lax = policy_with(vec![rule(
            RetentionScope::OriginalContent,
            Retention::Indefinite,
        )]);
        let medium = policy_with(vec![rule(
            RetentionScope::OriginalContent,
            Retention::Duration { days: 30 },
        )]);
        let resolved = resolve_retention([&strict, &lax, &medium]);
        assert_eq!(
            resolved[&RetentionScope::OriginalContent],
            Retention::ZeroRetention,
        );
    }

    #[test]
    fn smaller_duration_wins() {
        let week = policy_with(vec![rule(
            RetentionScope::RedactedOutput,
            Retention::Duration { days: 7 },
        )]);
        let month = policy_with(vec![rule(
            RetentionScope::RedactedOutput,
            Retention::Duration { days: 30 },
        )]);
        let resolved = resolve_retention([&month, &week]);
        assert_eq!(
            resolved[&RetentionScope::RedactedOutput],
            Retention::Duration { days: 7 },
        );
    }

    #[test]
    fn duration_beats_indefinite() {
        let bounded = policy_with(vec![rule(
            RetentionScope::AuditLogs,
            Retention::Duration { days: 365 },
        )]);
        let lax = policy_with(vec![rule(
            RetentionScope::AuditLogs,
            Retention::Indefinite,
        )]);
        let resolved = resolve_retention([&lax, &bounded]);
        assert_eq!(
            resolved[&RetentionScope::AuditLogs],
            Retention::Duration { days: 365 },
        );
    }

    #[test]
    fn order_does_not_matter() {
        let strict = policy_with(vec![rule(
            RetentionScope::OriginalContent,
            Retention::ZeroRetention,
        )]);
        let lax = policy_with(vec![rule(
            RetentionScope::OriginalContent,
            Retention::Indefinite,
        )]);
        let strict_first = resolve_retention([&strict, &lax]);
        let lax_first = resolve_retention([&lax, &strict]);
        assert_eq!(strict_first, lax_first);
    }

    #[test]
    fn resolves_each_scope_independently() {
        let a = policy_with(vec![
            rule(RetentionScope::OriginalContent, Retention::ZeroRetention),
            rule(
                RetentionScope::RedactedOutput,
                Retention::Duration { days: 30 },
            ),
        ]);
        let b = policy_with(vec![
            rule(RetentionScope::OriginalContent, Retention::Indefinite),
            rule(
                RetentionScope::RedactedOutput,
                Retention::Duration { days: 7 },
            ),
        ]);
        let resolved = resolve_retention([&a, &b]);
        assert_eq!(
            resolved[&RetentionScope::OriginalContent],
            Retention::ZeroRetention,
        );
        assert_eq!(
            resolved[&RetentionScope::RedactedOutput],
            Retention::Duration { days: 7 },
        );
    }

    #[test]
    fn retention_ord_total() {
        // Pin the strictness order so a future reorder of variant
        // declarations gets caught here.
        assert!(Retention::ZeroRetention < Retention::Duration { days: 1 });
        assert!(Retention::Duration { days: 1 } < Retention::Duration { days: 2 });
        assert!(Retention::Duration { days: u64::MAX } < Retention::Indefinite);
    }
}
