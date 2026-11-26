//! Request validation the label catalog depends on: every scope a
//! rule references is one its own policy declares.

use std::collections::HashSet;

use elide::{Error, ErrorKind, Result};
use elide_schema::policy::{Policy, PolicyRule, Predicate};

/// Reject a request whose rule references a [`LabelScope`] name
/// its own policy didn't declare.
///
/// Scopes are local to the policy that owns them (strict
/// per-policy namespace). A rule inside policy A can reference
/// only scopes declared in policy A's own [`scopes`]: not scopes
/// declared by policy B. Also rejects a policy declaring the same
/// scope name twice. Runs before catalog compilation
/// so an authoring typo (`"gdpr_arcticle_9"`) surfaces as a
/// [`Configuration`](ErrorKind::Configuration) error at request
/// validation time, not as a silent underfire at apply time.
///
/// [`LabelScope`]: elide_schema::policy::LabelScope
/// [`scopes`]: elide_schema::policy::Policy::scopes
pub(super) fn validate_scope_references(policies: &[Policy]) -> Result<()> {
    for policy in policies {
        let mut known: HashSet<&str> = HashSet::new();
        for declared in &policy.scopes {
            // Duplicate names would make a `LabelInScope` rule
            // resolve one labelset while `label_scope()` unions
            // both, so recognition and redaction would disagree
            // about what the name means.
            if !known.insert(declared.name.as_str()) {
                return Err(Error::new(
                    ErrorKind::Configuration,
                    format!(
                        "policy `{}` declares scope `{}` more than once; scope names \
                         must be unique within a policy",
                        policy.id,
                        declared.name.as_str(),
                    ),
                ));
            }
        }
        for rule in &policy.rules {
            for attachment in rule.attachments() {
                check_predicate_scopes(&attachment.predicate, &known, policy, rule)?;
            }
        }
        // Detected but unredactable: the policy scopes no labels
        // for the operators it carries, so its rules match nothing
        // while another policy's labels still reach it.
        if policy.scopes_nothing_it_redacts() {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "policy `{}` declares scopes but no labels in any of them, so its \
                     operators can match nothing: entities another policy asks for \
                     still reach it and go unredacted. Name the labels the policy \
                     covers.",
                    policy.id,
                ),
            ));
        }
    }
    Ok(())
}

/// Walk a predicate tree; every [`Predicate::LabelInScope`] leaf
/// must name a scope declared by the enclosing policy. Returns
/// the first unknown reference with policy + rule context for the
/// error message.
fn check_predicate_scopes(
    predicate: &Predicate,
    known: &HashSet<&str>,
    policy: &Policy,
    rule: &PolicyRule,
) -> Result<()> {
    match predicate {
        Predicate::LabelInScope { scope } if !known.contains(scope.as_str()) => Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "policy `{}` rule `{}` references unknown label scope `{}`: \
                 the enclosing policy declares no `LabelScope` with that name",
                policy.id, rule.id, scope,
            ),
        )),
        Predicate::All { all } => all
            .iter()
            .try_for_each(|p| check_predicate_scopes(p, known, policy, rule)),
        Predicate::Any { any } => any
            .iter()
            .try_for_each(|p| check_predicate_scopes(p, known, policy, rule)),
        Predicate::Not { not } => check_predicate_scopes(not, known, policy, rule),
        _ => Ok(()),
    }
}
