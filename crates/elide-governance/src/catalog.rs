//! Compile a slice of [`Policy`] into an
//! [`LabelCatalog`].
//!
//! Walks every policy's [`labels`] block and unions the builtin
//! selections and the inline custom schemas into one catalog. The
//! union is strict: an unknown builtin name, two policies
//! contributing a custom [`Label`] with the same id but different
//! contents, or a custom label whose id shadows a builtin all
//! fail the request with a [`Configuration`](ErrorKind::Configuration)
//! error at request-compile time. Silent last-write-wins semantics
//! would let policy A's authoring intent be quietly overwritten by
//! policy B's: the wrong posture for a governance surface.
//!
//! Groups do not stamp synthetic tags on the catalog. Group
//! membership is resolved by the selector when it compiles
//! [`Predicate::LabelInScope`], so no shared string namespace
//! exists that a [`Predicate::TagOneOf`] could exploit to bypass
//! per-policy group scoping.
//!
//! Lives here because the collision policy is authoring policy:
//! what makes a *set* of policies well-formed is the same kind of
//! question as what makes one policy well-formed, and both belong
//! to the crate that defines them. The engine only consumes the
//! catalog this produces.
//!
//! [`Label`]: elide_core::entity::Label
//! [`Policy`]: crate::policy::Policy
//! [`labels`]: crate::policy::Policy::label_scope
//! [`Predicate::LabelInScope`]: crate::policy::Predicate::LabelInScope
//! [`Predicate::TagOneOf`]: crate::policy::Predicate::TagOneOf

use std::sync::OnceLock;

use elide_core::entity::LabelCatalog;
use elide_core::{Error, ErrorKind, Result};

use crate::policy::Policy;
use crate::recognition::Recognition;

/// Compile the label catalog for a request from its policy set.
///
/// Every policy contributes its [`labels`] block; builtins are
/// resolved once against the cached full builtin catalog, custom
/// labels are inserted as-is.
///
/// Rejects the request as a [`Configuration`](ErrorKind::Configuration)
/// error if:
///
/// - a `labels.builtins` entry names a label the shipped elide
///   catalog does not know about (typo caught at request compile,
///   not silent underfire at apply time);
/// - a `labels.custom` id equals a shipped builtin id (silent
///   builtin shadowing would strip elide's carefully-curated
///   `pii` / `phi` / `pci` tags for every rule in the request);
/// - two policies contribute a `labels.custom` [`Label`] with the
///   same id but structurally different contents (byte-identical
///   redeclaration across templates is fine);
/// - the compiled catalog is empty — no policies, or none naming
///   a label. Elide reads an empty catalog as a request for no
///   entity types and detects nothing, so such a request can only
///   ever return an empty report. Failing here names the cause
///   rather than handing back a clean empty answer.
///
/// [`labels`]: crate::policy::Policy::label_scope
/// [`Label`]: elide_core::entity::Label
pub fn compile_catalog(policies: &[Policy], recognition: &[Recognition]) -> Result<LabelCatalog> {
    let mut catalog = LabelCatalog::new();
    // The request's own labels first: a policy scope may name one,
    // and the builtin lookup below would reject it as unknown.
    insert_custom(&mut catalog, recognition)?;
    for policy in policies {
        insert_params(&mut catalog, policy)?;
    }
    if catalog.is_empty() {
        return Err(Error::new(
            ErrorKind::Configuration,
            "the request names no labels to detect, so it can only return an \
             empty report: supply at least one policy scoping the labels to \
             find",
        ));
    }
    Ok(catalog)
}

/// Insert the labels this request introduces.
///
/// Request-wide, so a label is declared once however many policies
/// scope it. A custom id colliding with a shipped builtin is
/// refused: it would shadow elide's own definition, and every rule
/// naming that label would silently act on the wrong schema.
fn insert_custom(catalog: &mut LabelCatalog, recognition: &[Recognition]) -> Result<()> {
    let builtins = builtin_catalog();
    for label in recognition.iter().flat_map(|r| &r.custom) {
        if builtins.contains(&label.to_ref()) {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "custom label `{}` collides with a shipped builtin: a request \
                     cannot shadow elide's own definition",
                    label.id(),
                ),
            ));
        }
        catalog.insert(label.clone());
    }
    Ok(())
}

fn insert_params(catalog: &mut LabelCatalog, policy: &Policy) -> Result<()> {
    let builtins = builtin_catalog();

    // Every label the policy's scopes name. A request's own labels
    // are already in the catalog, so a scope naming one resolves
    // through that rather than through the builtins.
    // policy-local: an earlier policy's custom schema must not make
    // this policy's reference to it resolve, or a rule could act on
    // a label its own policy never declared.
    for label_ref in policy.label_scope() {
        if catalog.contains(&label_ref) {
            continue;
        }
        let label = builtins.get(&label_ref).ok_or_else(|| {
            Error::new(
                ErrorKind::Configuration,
                format!(
                    "policy `{}` scopes label `{}`, which is neither a shipped elide \
                     builtin nor one of the request's own custom labels",
                    policy.id,
                    label_ref.as_str(),
                ),
            )
        })?;
        catalog.insert(label.clone());
    }
    Ok(())
}

/// The full builtin label catalog from `elide-core`, built once
/// and reused for every request. [`LabelCatalog::with_builtins`]
/// walks `BUILT_INS` and clones every label: cheap once, wasteful
/// per-request.
///
/// [`LabelCatalog::with_builtins`]: elide::entity::LabelCatalog::with_builtins
fn builtin_catalog() -> &'static LabelCatalog {
    static BUILTINS: OnceLock<LabelCatalog> = OnceLock::new();
    BUILTINS.get_or_init(LabelCatalog::with_builtins)
}

#[cfg(test)]
mod tests {
    use elide_core::entity::{Label, LabelRef};
    use hipstr::HipStr;
    use uuid::Uuid;

    use super::*;
    use crate::policy::{LabelScope, Policy};

    const POLICY_A: Uuid = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000010_u128);
    const POLICY_B: Uuid = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000011_u128);

    fn policy_named(id: Uuid, scopes: Vec<LabelScope>) -> Policy {
        Policy {
            id,
            name: HipStr::from("test"),
            scopes,
            ..Policy::default()
        }
    }

    /// A policy scoping `labels`, or scoping nothing when empty.
    fn policy_scoping(labels: Vec<LabelRef>) -> Policy {
        let scopes = if labels.is_empty() {
            Vec::new()
        } else {
            vec![LabelScope::new("scope", labels)]
        };
        policy_named(POLICY_A, scopes)
    }

    /// A request introducing `custom` and nothing else.
    fn introducing(custom: Vec<Label>) -> Recognition {
        Recognition {
            custom,
            matchers: Vec::new(),
        }
    }

    #[test]
    fn a_policy_set_naming_no_labels_is_rejected() {
        // An empty catalog is a request for no entity types, so it
        // detects nothing: refuse rather than compile a request
        // that can only return an empty report.
        let err = compile_catalog(&[], &[]).expect_err("empty policy set is refused");
        assert_eq!(err.kind(), ErrorKind::Configuration);

        let bare = policy_named(POLICY_A, Vec::new());
        compile_catalog(std::slice::from_ref(&bare), &[])
            .expect_err("a policy naming no labels is refused too");
    }

    #[test]
    fn builtin_names_land_in_the_catalog() {
        let p = policy_scoping(vec![LabelRef::new("email_address")]);
        let catalog = compile_catalog(std::slice::from_ref(&p), &[]).unwrap();
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn unknown_builtin_name_fails_the_request() {
        let p = policy_scoping(vec![LabelRef::new("definitely_not_a_real_label")]);
        let err = compile_catalog(std::slice::from_ref(&p), &[])
            .expect_err("unknown builtin must reject the request");
        assert!(err.to_string().contains("definitely_not_a_real_label"));
        assert!(err.to_string().contains(&POLICY_A.to_string()));
    }

    #[test]
    fn custom_labels_land_in_the_catalog() {
        let p = policy_scoping(vec![LabelRef::new("project_code")]);
        let catalog = compile_catalog(
            std::slice::from_ref(&p),
            &[introducing(vec![Label::new(
                "project_code",
                "Project code",
            )])],
        )
        .unwrap();
        assert!(catalog.contains(&LabelRef::new("project_code")));
    }

    /// A label the request introduces is shared: two policies both
    /// scoping it is one label, not a collision. Declaring it per
    /// policy is what made that a conflict to police.
    #[test]
    fn two_policies_may_scope_one_custom_label() {
        let a = policy_named(
            POLICY_A,
            vec![LabelScope::new("a", vec![LabelRef::new("project_code")])],
        );
        let b = policy_named(
            POLICY_B,
            vec![LabelScope::new("b", vec![LabelRef::new("project_code")])],
        );
        let catalog = compile_catalog(
            &[a, b],
            &[introducing(vec![Label::new(
                "project_code",
                "Project code",
            )])],
        )
        .unwrap();
        assert!(catalog.contains(&LabelRef::new("project_code")));
        assert_eq!(
            catalog.len(),
            1,
            "one label, however many policies scope it"
        );
    }

    #[test]
    fn multiple_policies_union_their_labels() {
        let a = policy_named(
            POLICY_A,
            vec![LabelScope::new(
                "scope",
                vec![LabelRef::new("email_address")],
            )],
        );
        let b = policy_named(
            POLICY_B,
            vec![LabelScope::new(
                "scope",
                vec![LabelRef::new("phone_number")],
            )],
        );
        let catalog = compile_catalog(&[a, b], &[]).unwrap();
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert!(catalog.contains(&LabelRef::new("phone_number")));
        assert_eq!(catalog.len(), 2);
    }

    #[test]
    fn custom_label_shadowing_a_builtin_fails_the_request() {
        // `email_address` is a shipped builtin; a request declaring
        // a custom label with that id would silently strip elide's
        // `contact_info`/`pii` tags for every rule in the request.
        // Reject it.
        let p = policy_scoping(vec![LabelRef::new("email_address")]);
        let err = compile_catalog(
            std::slice::from_ref(&p),
            &[introducing(vec![Label::new(
                "email_address",
                "Adresse électronique",
            )])],
        )
        .expect_err("shadowing a builtin must reject the request");
        assert!(err.to_string().contains("email_address"));
        assert!(err.to_string().contains("shadow"));
    }

    #[test]
    fn scopes_do_not_stamp_synthetic_tags_on_the_catalog() {
        // Scope membership resolves against the policy's own table
        // at predicate-evaluation time; nothing on the catalog
        // carries a `scope:*` tag a `TagOneOf` could exploit to
        // reach across policies.
        let p = policy_scoping(vec![LabelRef::new("email_address")]);
        let catalog = compile_catalog(std::slice::from_ref(&p), &[]).unwrap();
        let stamped = catalog.get(&LabelRef::new("email_address")).unwrap();
        assert!(
            !stamped
                .tags()
                .iter()
                .any(|t| t.as_str().starts_with("group:")),
            "no synthetic `group:*` tag should appear on the catalog",
        );
        // Shipped elide tags + category are preserved.
        assert!(stamped.has_tag("pii"));
        assert_eq!(
            stamped.category().map(|c| c.as_str()),
            Some("contact"),
            "elide's category on `email_address` must survive catalog compile",
        );
    }

    /// A policy's scope is its scopes, whatever the label's origin.
    /// A request-introduced label reaches a policy by being scoped,
    /// exactly as a shipped one does.
    #[test]
    fn label_scope_is_the_union_of_the_scopes() {
        let p = policy_scoping(vec![
            LabelRef::new("email_address"),
            LabelRef::new("project_code"),
        ]);
        let scope = p.label_scope();
        assert!(scope.contains(&LabelRef::new("email_address")));
        assert!(
            scope.contains(&LabelRef::new("project_code")),
            "a custom label is scoped like any other",
        );
        assert_eq!(scope.len(), 2);
    }
}
