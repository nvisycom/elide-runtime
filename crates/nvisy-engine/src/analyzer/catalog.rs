//! Compile a slice of [`PolicyDefinition`] into an
//! [`elide_core::entity::LabelCatalog`].
//!
//! Walks every policy's [`labels`] block, unions the builtin
//! selections and the inline custom schemas into one catalog,
//! then stamps a `group:<policy_id>:<name>` synthetic tag on
//! every label listed in each policy's [`groups`]. Kept
//! engine-side so the cached `with_builtins` lookup, the
//! warn-and-skip policy for unknown builtin names, and the group
//! tag synthesis stay out of `nvisy-policy`.
//!
//! [`PolicyDefinition`]: nvisy_schema::policy::PolicyDefinition
//! [`labels`]: nvisy_schema::policy::PolicyDefinition::labels
//! [`groups`]: nvisy_schema::policy::PolicyDefinition::groups

use std::sync::OnceLock;

use elide_core::entity::LabelCatalog;
use nvisy_schema::policy::{LabelGroup, Labels, PolicyDefinition};
use uuid::Uuid;

/// Build the synthetic tag a `LabelGroup` compiles to:
/// `group:<policy_id>:<group_name>`. Scoping by `policy_id` keeps
/// two policies that both declare `hipaa_18` with different
/// labelsets from stepping on each other.
///
/// Shared between catalog synthesis and the selector's rewrite
/// of `Predicate::LabelInGroup` so the two sides can't drift.
pub(crate) fn synthetic_group_tag(policy_id: Uuid, group_name: &str) -> String {
    format!("group:{policy_id}:{group_name}")
}

/// Compile the label catalog for a request from its policy set.
///
/// Every policy contributes its [`labels`] block; builtins are
/// resolved once against the cached full builtin catalog, custom
/// labels are inserted as-is. Names that collide across policies
/// or between builtins and customs follow
/// [`LabelCatalog::insert`] semantics: last write wins.
///
/// After label union, each policy's [`groups`] stamp a
/// `group:<policy_id>:<name>` tag onto every listed label
/// present in the catalog — the compile-time rewrite that makes
/// [`Predicate::LabelInGroup { group }`] lower to
/// [`TagOneOf`]`{ tags: ["group:<policy_id>:<name>"] }`.
/// A label listed in a group but absent from the catalog is
/// silently skipped; groups can safely reference labels this
/// build doesn't ship (e.g. modality-gated ones).
///
/// Unknown builtin names log a warning and are skipped — typos
/// shouldn't fail the request. Unknown group *names* are
/// separately validated by
/// `pipeline::orchestrator::validate_group_references`.
///
/// [`labels`]: nvisy_schema::policy::PolicyDefinition::labels
/// [`groups`]: nvisy_schema::policy::PolicyDefinition::groups
/// [`LabelCatalog::insert`]: elide_core::entity::LabelCatalog::insert
/// [`Predicate::LabelInGroup { group }`]: nvisy_schema::policy::predicate::Predicate::LabelInGroup
/// [`TagOneOf`]: nvisy_schema::policy::predicate::Predicate::TagOneOf
pub(crate) fn compile_catalog(policies: &[PolicyDefinition]) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    for policy in policies {
        insert_params(&mut catalog, &policy.labels);
    }
    for policy in policies {
        apply_group_tags(&mut catalog, policy.id, &policy.groups);
    }
    catalog
}

/// Stamp `group:<policy_id>:<name>` on every label a group lists
/// that the catalog carries. Preserves the label's existing tag
/// list — elide's `with_tags` builder *replaces* the tag list,
/// so we read the current tags, append the synthetic one, and
/// re-insert (last-write-wins on the catalog side).
fn apply_group_tags(catalog: &mut LabelCatalog, policy_id: Uuid, groups: &[LabelGroup]) {
    for group in groups {
        let synthetic_tag = synthetic_group_tag(policy_id, group.name.as_str());
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

    const FIXED_POLICY_ID: Uuid = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000001_u128);

    fn policy(labels: Labels, groups: Vec<LabelGroup>) -> PolicyDefinition {
        PolicyDefinition {
            id: FIXED_POLICY_ID,
            name: HipStr::from("test"),
            description: None,
            when: None,
            labels,
            groups,
            rules: Vec::new(),
            fallback: None,
            retention: Vec::new(),
        }
    }

    fn policy_with_labels(labels: Labels) -> PolicyDefinition {
        policy(labels, Vec::new())
    }

    #[test]
    fn empty_policy_set_yields_empty_catalog() {
        assert!(compile_catalog(&[]).is_empty());
    }

    #[test]
    fn policy_with_no_labels_contributes_nothing() {
        let p = policy_with_labels(Labels::default());
        assert!(compile_catalog(std::slice::from_ref(&p)).is_empty());
    }

    #[test]
    fn builtin_names_land_in_the_catalog() {
        let p = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        });
        let catalog = compile_catalog(std::slice::from_ref(&p));
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn unknown_builtin_names_skip_instead_of_failing() {
        let p = policy_with_labels(Labels {
            builtins: vec![
                LabelRef::new("email_address"),
                LabelRef::new("definitely_not_a_real_label"),
            ],
            custom: Vec::new(),
        });
        let catalog = compile_catalog(std::slice::from_ref(&p));
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert!(!catalog.contains(&LabelRef::new("definitely_not_a_real_label")));
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn custom_labels_land_in_the_catalog() {
        let p = policy_with_labels(Labels {
            builtins: Vec::new(),
            custom: vec![Label::new("project_code", "Project code")],
        });
        let catalog = compile_catalog(std::slice::from_ref(&p));
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
        let catalog = compile_catalog(&[a, b]);
        assert!(catalog.contains(&LabelRef::new("email_address")));
        assert!(catalog.contains(&LabelRef::new("phone_number")));
        assert_eq!(catalog.len(), 2);
    }

    #[test]
    fn group_stamps_scoped_synthetic_tag_and_preserves_existing_tags() {
        let group = LabelGroup {
            name: HipStr::from("contact_sweep"),
            description: None,
            labels: vec![LabelRef::new("email_address")],
        };
        let p = policy(
            Labels {
                builtins: vec![LabelRef::new("email_address")],
                custom: Vec::new(),
            },
            vec![group],
        );
        let expected_tag = synthetic_group_tag(FIXED_POLICY_ID, "contact_sweep");
        let catalog = compile_catalog(std::slice::from_ref(&p));
        let stamped = catalog.get(&LabelRef::new("email_address")).unwrap();
        assert!(stamped.has_tag(&expected_tag));
        // Elide's builtin email_address carries `contact_info` and `pii`;
        // synthesis appends, doesn't replace.
        assert!(stamped.has_tag("pii"));
        assert!(stamped.has_tag("contact_info"));
    }

    #[test]
    fn group_referencing_absent_label_is_silently_skipped() {
        let group = LabelGroup {
            name: HipStr::from("nothing_present"),
            description: None,
            labels: vec![LabelRef::new("email_address")],
        };
        // No policy labels are declared; the group's target label
        // is absent from the catalog, so the group is a no-op.
        let p = policy(Labels::default(), vec![group]);
        let catalog = compile_catalog(std::slice::from_ref(&p));
        assert!(catalog.is_empty());
    }

    #[test]
    fn repeated_group_application_is_idempotent() {
        // The same group appearing twice on a policy stamps the
        // tag once (has_tag short-circuits the second attempt).
        let group = LabelGroup {
            name: HipStr::from("dup"),
            description: None,
            labels: vec![LabelRef::new("email_address")],
        };
        let p = policy(
            Labels {
                builtins: vec![LabelRef::new("email_address")],
                custom: Vec::new(),
            },
            vec![group.clone(), group],
        );
        let expected_tag = synthetic_group_tag(FIXED_POLICY_ID, "dup");
        let catalog = compile_catalog(std::slice::from_ref(&p));
        let stamped = catalog.get(&LabelRef::new("email_address")).unwrap();
        assert_eq!(
            stamped
                .tags()
                .iter()
                .filter(|t| t.as_str() == expected_tag)
                .count(),
            1,
        );
    }

    #[test]
    fn two_policies_with_same_group_name_get_distinct_tags() {
        // Strict scoping: two policies declaring `hipaa_18` with
        // different labelsets don't step on each other — each
        // stamps its own `group:<policy_id>:hipaa_18` tag.
        let policy_a_id = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000010_u128);
        let policy_b_id = Uuid::from_u128(0x01234567_89ab_7000_8000_000000000011_u128);
        let mut a = policy(
            Labels {
                builtins: vec![LabelRef::new("email_address")],
                custom: Vec::new(),
            },
            vec![LabelGroup {
                name: HipStr::from("hipaa_18"),
                description: None,
                labels: vec![LabelRef::new("email_address")],
            }],
        );
        a.id = policy_a_id;
        let mut b = policy(
            Labels {
                builtins: vec![LabelRef::new("email_address")],
                custom: Vec::new(),
            },
            vec![LabelGroup {
                name: HipStr::from("hipaa_18"),
                description: None,
                labels: vec![LabelRef::new("email_address")],
            }],
        );
        b.id = policy_b_id;
        let catalog = compile_catalog(&[a, b]);
        let stamped = catalog.get(&LabelRef::new("email_address")).unwrap();
        assert!(stamped.has_tag(&synthetic_group_tag(policy_a_id, "hipaa_18")));
        assert!(stamped.has_tag(&synthetic_group_tag(policy_b_id, "hipaa_18")));
    }
}
