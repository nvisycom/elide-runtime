//! Compile a slice of [`PolicyDefinition`] into an
//! [`elide_core::entity::LabelCatalog`].
//!
//! Walks every policy's [`labels`] block, unions the builtin
//! selections and the inline custom schemas into one catalog.
//! Kept engine-side so the cached `with_builtins` lookup and the
//! warn-and-skip policy for unknown builtin names stay out of
//! `nvisy-policy`.
//!
//! [`PolicyDefinition`]: nvisy_schema::policy::PolicyDefinition
//! [`labels`]: nvisy_schema::policy::PolicyDefinition::labels

use std::sync::OnceLock;

use elide_core::entity::LabelCatalog;
use nvisy_schema::policy::{Labels, PolicyDefinition};

/// Compile the label catalog for a request from its policy set.
///
/// Every policy contributes its [`labels`] block; builtins are
/// resolved once against the cached full builtin catalog, custom
/// labels are inserted as-is. Names that collide across policies
/// or between builtins and customs follow
/// [`LabelCatalog::insert`] semantics: last write wins.
///
/// Unknown builtin names log a warning and are skipped — typos
/// shouldn't fail the request.
///
/// [`labels`]: nvisy_schema::policy::PolicyDefinition::labels
/// [`LabelCatalog::insert`]: elide_core::entity::LabelCatalog::insert
pub(crate) fn compile_catalog(policies: &[PolicyDefinition]) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    for policy in policies {
        insert_params(&mut catalog, &policy.labels);
    }
    catalog
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
        assert!(compile_catalog(&[]).is_empty());
    }

    #[test]
    fn policy_with_no_labels_contributes_nothing() {
        let policy = policy_with_labels(Labels::default());
        assert!(compile_catalog(std::slice::from_ref(&policy)).is_empty());
    }

    #[test]
    fn builtin_names_land_in_the_catalog() {
        let policy = policy_with_labels(Labels {
            builtins: vec![LabelRef::new("email_address")],
            custom: Vec::new(),
        });
        let catalog = compile_catalog(std::slice::from_ref(&policy));
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
        let catalog = compile_catalog(std::slice::from_ref(&policy));
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
        let catalog = compile_catalog(std::slice::from_ref(&policy));
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
}
