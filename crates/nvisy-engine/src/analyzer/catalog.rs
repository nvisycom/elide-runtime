//! Compile a slice of [`PolicyDefinition`] and the request's
//! [`LabelGroup`]s into an [`elide_core::entity::LabelCatalog`].
//!
//! Walks every policy's [`labels`] block, unions the builtin
//! selections and the inline custom schemas into one catalog,
//! then stamps a `group:<name>` synthetic tag on every label
//! listed in each [`LabelGroup`]. Kept engine-side so the cached
//! `with_builtins` lookup, the warn-and-skip policy for unknown
//! builtin names, and the group tag synthesis stay out of
//! `nvisy-policy`.
//!
//! [`PolicyDefinition`]: nvisy_schema::policy::PolicyDefinition
//! [`LabelGroup`]: nvisy_schema::policy::LabelGroup
//! [`labels`]: nvisy_schema::policy::PolicyDefinition::labels

use std::sync::OnceLock;

use elide_core::entity::LabelCatalog;
use nvisy_schema::policy::{LabelGroup, Labels, PolicyDefinition};

/// Prefix elide-side synthetic tags carry so a `LabelGroup`
/// named `hipaa_18` becomes the tag `group:hipaa_18` on every
/// listed label. Keeps synthetic tags in their own namespace,
/// away from elide's data-category tags (`pii`, `phi`, …).
pub(crate) const GROUP_TAG_PREFIX: &str = "group:";

/// Compile the label catalog for a request from its policy set
/// and its [`LabelGroup`]s.
///
/// Every policy contributes its [`labels`] block; builtins are
/// resolved once against the cached full builtin catalog, custom
/// labels are inserted as-is. Names that collide across policies
/// or between builtins and customs follow
/// [`LabelCatalog::insert`] semantics: last write wins.
///
/// After label union, each [`LabelGroup`] stamps a
/// `group:<name>` tag onto every listed label present in the
/// catalog — the compile-time rewrite that makes
/// [`Predicate::LabelInGroup { group }`] lower to
/// [`TagOneOf`]`{ tags: ["group:<name>"] }`. A label listed in a
/// group but absent from the catalog is silently skipped; groups
/// can safely reference labels this build doesn't ship (e.g.
/// modality-gated ones).
///
/// Unknown builtin names log a warning and are skipped — typos
/// shouldn't fail the request. Unknown group *names* are
/// separately validated by [`validate_groups_referenced_in_policies`].
///
/// [`LabelGroup`]: nvisy_schema::policy::LabelGroup
/// [`labels`]: nvisy_schema::policy::PolicyDefinition::labels
/// [`LabelCatalog::insert`]: elide_core::entity::LabelCatalog::insert
/// [`Predicate::LabelInGroup { group }`]: nvisy_schema::policy::predicate::Predicate::LabelInGroup
/// [`TagOneOf`]: nvisy_schema::policy::predicate::Predicate::TagOneOf
pub(crate) fn compile_catalog(
    policies: &[PolicyDefinition],
    groups: &[LabelGroup],
) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    for policy in policies {
        insert_params(&mut catalog, &policy.labels);
    }
    apply_group_tags(&mut catalog, groups);
    catalog
}

/// Stamp `group:<name>` on every label a group lists that the
/// catalog carries. Preserves the label's existing tag list —
/// elide's `with_tags` builder *replaces* the tag list, so we
/// read the current tags, append the synthetic one, and
/// re-insert (last-write-wins on the catalog side).
fn apply_group_tags(catalog: &mut LabelCatalog, groups: &[LabelGroup]) {
    for group in groups {
        let synthetic_tag = format!("{GROUP_TAG_PREFIX}{}", group.name);
        for label_ref in &group.labels {
            let Some(label) = catalog.get(label_ref) else {
                continue;
            };
            if label.has_tag(&synthetic_tag) {
                continue;
            }
            let merged: Vec<_> = label
                .tags()
                .iter()
                .cloned()
                .chain(std::iter::once(synthetic_tag.clone().into()))
                .collect();
            let stamped = label.clone().with_tags(merged);
            catalog.insert(stamped);
        }
    }
}

fn insert_params(catalog: &mut LabelCatalog, params: &Labels) {
    let builtins = builtin_catalog();
    for label_ref in &params.builtins {
        match builtins.get(label_ref) {
            Some(label) => {
                catalog.insert(label.clone());
            }
            None => {
                tracing::warn!(
                    target: "engine::analyzer",
                    label = label_ref.as_str(),
                    "unknown builtin label name in policy catalog; skipping",
                );
            }
        }
    }
    for label in &params.custom {
        catalog.insert(label.clone());
    }
}

/// The full builtin label catalog from `elide-core`, built once
/// and reused for every request. [`LabelCatalog::with_builtins`]
/// walks `BUILT_INS` and clones every label — cheap once, wasteful
/// per-request.
///
/// [`LabelCatalog::with_builtins`]: elide_core::entity::LabelCatalog::with_builtins
fn builtin_catalog() -> &'static LabelCatalog {
    static BUILTINS: OnceLock<LabelCatalog> = OnceLock::new();
    BUILTINS.get_or_init(LabelCatalog::with_builtins)
}

#[cfg(test)]
mod tests {
    use elide_core::entity::{Label, LabelRef};
    use hipstr::HipStr;
    use nvisy_schema::policy::LabelGroup;
    use uuid::Uuid;

    use super::*;

    fn policy_with_labels(labels: Labels) -> PolicyDefinition {
        PolicyDefinition {
            id: Uuid::now_v7(),
            name: HipStr::from("test"),
            description: None,
            when: None,
            labels,
            rules: Vec::new(),
            fallback: None,
            retention: Vec::new(),
        }
    }

    #[test]
    fn empty_policy_set_yields_empty_catalog() {
        assert!(compile_catalog(&[], &[]).is_empty());
    }

    #[test]
    fn policy_with_no_labels_contributes_nothing() {
        let policy = policy_with_labels(Labels::default());
        assert!(compile_catalog(std::slice::from_ref(&policy), &[]).is_empty());
    }

    #[test]
    fn builtin_names_land_in_the_catalog() {
        let policy = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        });
        let catalog = compile_catalog(std::slice::from_ref(&policy), &[]);
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn unknown_builtin_names_skip_instead_of_failing() {
        let policy = policy_with_labels(Labels {
            builtins: vec![
                LabelRef::new("email_address"),
                LabelRef::new("definitely_not_a_real_label"),
            ],
            custom: Vec::new(),
        });
        let catalog = compile_catalog(std::slice::from_ref(&policy), &[]);
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert!(!catalog.contains(&LabelRef::new("definitely_not_a_real_label")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn custom_labels_land_in_the_catalog() {
        let policy = policy_with_labels(Labels {
            builtins: Vec::new(),
            custom: vec![Label::new("project_code", "Project code")],
        });
        let catalog = compile_catalog(std::slice::from_ref(&policy), &[]);
        assert!(catalog.contains(&LabelRef::new("project_code")));
    }

    #[test]
    fn multiple_policies_union_their_labels() {
        let a = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        });
        let b = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("phone_number")],
            custom: Vec::new(),
        });
        let catalog = compile_catalog(&[a, b], &[]);
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert!(catalog.contains(&LabelRef::new("phone_number")));
        assert_eq!(catalog.len(), 2);
    }

    #[test]
    fn group_stamps_synthetic_tag_and_preserves_existing_tags() {
        let policy = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        });
        let group = LabelGroup {
            name: HipStr::from("contact_sweep"),
            description: None,
            labels: vec![LabelRef::new("email_address")],
        };
        let catalog = compile_catalog(std::slice::from_ref(&policy), &[group]);
        let stamped = catalog.get(&LabelRef::new("email_address")).unwrap();
        assert!(stamped.has_tag("group:contact_sweep"));
        // Elide's builtin email_address carries `contact_info` and `pii`;
        // synthesis appends, doesn't replace.
        assert!(stamped.has_tag("pii"));
        assert!(stamped.has_tag("contact_info"));
    }

    #[test]
    fn group_referencing_absent_label_is_silently_skipped() {
        // No policy contributes labels; the group's target label
        // isn't in the catalog, so the group is a no-op.
        let group = LabelGroup {
            name: HipStr::from("nothing_present"),
            description: None,
            labels: vec![LabelRef::new("email_address")],
        };
        let catalog = compile_catalog(&[], &[group]);
        assert!(catalog.is_empty());
    }

    #[test]
    fn repeated_group_application_is_idempotent() {
        // Two groups with the same name applied twice should not
        // double-stamp the tag (has_tag short-circuits).
        let policy = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        });
        let group = LabelGroup {
            name: HipStr::from("dup"),
            description: None,
            labels: vec![LabelRef::new("email_address")],
        };
        let catalog = compile_catalog(std::slice::from_ref(&policy), &[group.clone(), group]);
        let stamped = catalog.get(&LabelRef::new("email_address")).unwrap();
        assert_eq!(
            stamped.tags().iter().filter(|t| t.as_str() == "group:dup").count(),
            1,
        );
    }
}
