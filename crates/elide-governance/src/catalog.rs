//! Compile a slice of [`PolicyDefinition`] into an
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
//! [`PolicyDefinition`]: crate::PolicyDefinition
//! [`labels`]: crate::PolicyDefinition::label_scope
//! [`Predicate::LabelInScope`]: crate::Predicate::LabelInScope
//! [`Predicate::TagOneOf`]: crate::Predicate::TagOneOf

use std::sync::OnceLock;

use elide_core::entity::LabelCatalog;
use elide_core::{Error, ErrorKind, Result};

use crate::PolicyDefinition;

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
///   redeclaration across templates is fine).
///
/// [`labels`]: crate::PolicyDefinition::label_scope
/// [`Label`]: elide_core::entity::Label
pub fn compile_catalog(policies: &[PolicyDefinition]) -> Result<LabelCatalog> {
    let mut catalog = LabelCatalog::new();
    for policy in policies {
        insert_params(&mut catalog, policy)?;
    }
    Ok(catalog)
}

fn insert_params(catalog: &mut LabelCatalog, policy: &PolicyDefinition) -> Result<()> {
    let builtins = builtin_catalog();

    // Customs first: a scope may name one, and the builtin lookup
    // below would otherwise reject it as unknown.
    for label in &policy.custom {
        if builtins.contains(&label.to_ref()) {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "policy `{}` declares custom label `{}` whose id collides with a \
                     shipped builtin: customs cannot shadow builtins",
                    policy.id,
                    label.id(),
                ),
            ));
        }
        if let Some(existing) = catalog.get(&label.to_ref())
            && existing != label
        {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "policy `{}` declares custom label `{}` that another policy in the \
                     same request already contributed with different contents",
                    policy.id,
                    label.id(),
                ),
            ));
        }
        catalog.insert(label.clone());
    }

    // Then every label the policy's scopes name. Resolution is
    // policy-local: an earlier policy's custom schema must not make
    // this policy's reference to it resolve, or a rule could act on
    // a label its own policy never declared.
    for label_ref in policy.label_scope() {
        if policy.custom.iter().any(|l| l.to_ref() == label_ref) {
            continue;
        }
        let label = builtins.get(&label_ref).ok_or_else(|| {
            Error::new(
                ErrorKind::Configuration,
                format!(
                    "policy `{}` scopes label `{}`, which is neither a shipped elide \
                     builtin nor one of the policy's own custom labels",
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
    use crate::{LabelScope, PolicyDefinition};

    const POLICY_A: Uuid = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000010_u128);
    const POLICY_B: Uuid = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000011_u128);

    fn policy_named(id: Uuid, custom: Vec<Label>, scopes: Vec<LabelScope>) -> PolicyDefinition {
        PolicyDefinition {
            id,
            name: HipStr::from("test"),
            description: None,
            template: None,
            scopes,
            custom,
            rules: Vec::new(),
            fallback: None,
        }
    }

    /// A policy whose vocabulary is one scope naming `builtins`,
    /// plus any inline `custom` schemas.
    fn policy_with_labels(builtins: Vec<LabelRef>, custom: Vec<Label>) -> PolicyDefinition {
        let scopes = if builtins.is_empty() {
            Vec::new()
        } else {
            vec![LabelScope::new("scope", builtins)]
        };
        policy_named(POLICY_A, custom, scopes)
    }

    #[test]
    fn empty_policy_set_yields_empty_catalog() {
        assert!(compile_catalog(&[]).unwrap().is_empty());
    }

    #[test]
    fn builtin_names_land_in_the_catalog() {
        let p = policy_with_labels(vec![LabelRef::new("email_address")], Vec::new());
        let catalog = compile_catalog(std::slice::from_ref(&p)).unwrap();
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn unknown_builtin_name_fails_the_request() {
        let p = policy_with_labels(
            vec![LabelRef::new("definitely_not_a_real_label")],
            Vec::new(),
        );
        let err = compile_catalog(std::slice::from_ref(&p))
            .expect_err("unknown builtin must reject the request");
        assert!(err.to_string().contains("definitely_not_a_real_label"));
        assert!(err.to_string().contains(&POLICY_A.to_string()));
    }

    #[test]
    fn custom_labels_land_in_the_catalog() {
        let p = policy_with_labels(Vec::new(), vec![Label::new("project_code", "Project code")]);
        let catalog = compile_catalog(std::slice::from_ref(&p)).unwrap();
        assert!(catalog.contains(&LabelRef::new("project_code")));
    }

    #[test]
    fn multiple_policies_union_their_labels() {
        let a = policy_named(
            POLICY_A,
            Vec::new(),
            vec![LabelScope::new(
                "scope",
                vec![LabelRef::new("email_address")],
            )],
        );
        let b = policy_named(
            POLICY_B,
            Vec::new(),
            vec![LabelScope::new(
                "scope",
                vec![LabelRef::new("phone_number")],
            )],
        );
        let catalog = compile_catalog(&[a, b]).unwrap();
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert!(catalog.contains(&LabelRef::new("phone_number")));
        assert_eq!(catalog.len(), 2);
    }

    #[test]
    fn custom_label_shadowing_a_builtin_fails_the_request() {
        // `email_address` is a shipped builtin; a policy that
        // declares a custom label with that id would silently strip
        // elide's `contact_info`/`pii` tags for every rule in the
        // request. Reject it.
        let p = policy_with_labels(
            Vec::new(),
            vec![Label::new("email_address", "Adresse électronique")],
        );
        let err = compile_catalog(std::slice::from_ref(&p))
            .expect_err("shadowing a builtin must reject the request");
        assert!(err.to_string().contains("email_address"));
        assert!(err.to_string().contains("shadow"));
    }

    #[test]
    fn same_custom_label_declared_identically_across_policies_is_fine() {
        // Two policies (templates, deployed side-by-side) that
        // both declare the same custom label with byte-identical
        // contents represent a shared vocabulary. Union cleanly.
        let label = Label::new("project_code", "Project code");
        let a = policy_named(
            POLICY_A,
            vec![label.clone()],
            vec![LabelScope::new("scope", Vec::new())],
        );
        let b = policy_named(
            POLICY_B,
            vec![label],
            vec![LabelScope::new("scope", Vec::new())],
        );
        let catalog = compile_catalog(&[a, b]).unwrap();
        assert!(catalog.contains(&LabelRef::new("project_code")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn same_custom_label_id_with_different_contents_fails_the_request() {
        // Two policies contributing `project_code` with different
        // descriptions is silent last-write-wins in the legacy
        // shape. Reject: the caller has a bug (glued two conflicting
        // templates) and picking a winner would silently misredact.
        let a = policy_named(
            POLICY_A,
            vec![Label::new("project_code", "Project code")],
            vec![LabelScope::new("scope", Vec::new())],
        );
        let b = policy_named(
            POLICY_B,
            vec![Label::new("project_code", "Legacy code")],
            vec![LabelScope::new("scope", Vec::new())],
        );
        let err = compile_catalog(&[a, b])
            .expect_err("conflicting custom labels must reject the request");
        assert!(err.to_string().contains("project_code"));
        assert!(err.to_string().contains(&POLICY_B.to_string()));
    }

    #[test]
    fn scopes_do_not_stamp_synthetic_tags_on_the_catalog() {
        // Scope membership resolves against the policy's own table
        // at predicate-evaluation time; nothing on the catalog
        // carries a `scope:*` tag a `TagOneOf` could exploit to
        // reach across policies.
        let p = policy_named(
            POLICY_A,
            Vec::new(),
            vec![LabelScope::new(
                "scope",
                vec![LabelRef::new("email_address")],
            )],
        );
        let catalog = compile_catalog(std::slice::from_ref(&p)).unwrap();
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

    #[test]
    fn label_scope_unions_scopes_and_customs() {
        let p = policy_with_labels(
            vec![
                LabelRef::new("email_address"),
                LabelRef::new("phone_number"),
            ],
            vec![Label::new("project_code", "Project code")],
        );
        let scope = p.label_scope();
        assert!(scope.contains(&LabelRef::new("email_address")));
        assert!(scope.contains(&LabelRef::new("phone_number")));
        assert!(scope.contains(&LabelRef::new("project_code")));
        assert_eq!(scope.len(), 3);
    }
}
