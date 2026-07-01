//! Cross-policy retention resolution.
//!
//! The engine's resolution strategy is "strictest wins": when
//! two policies both name a retention for the same scope, the
//! stricter [`Retention`] is kept. Different engines could
//! choose a different strategy (laxest wins, deployment-config
//! default); the choice lives here rather than in
//! [`nvisy_core`] so the core crate stays a frozen data model.
//!
//! Consumed by the run lifecycle at [`Engine::start_run`] (pin
//! `OriginalContent` per input) and [`Engine::apply_run`] (pin
//! `RedactedOutput` per output).
//!
//! [`Engine::start_run`]: crate::Engine::start_run
//! [`Engine::apply_run`]: crate::Engine::apply_run

use std::collections::HashMap;

use nvisy_schema::policy::{Policy, Retention, RetentionScope};

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
    use nvisy_schema::policy::RetentionPolicy;
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
        let lax = policy_with(vec![rule(RetentionScope::AuditLogs, Retention::Indefinite)]);
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
