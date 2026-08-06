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
use nvisy_schema::policy::{LabelCatalogParams, PolicyDefinition};

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

fn insert_params(catalog: &mut LabelCatalog, params: &LabelCatalogParams) {
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
