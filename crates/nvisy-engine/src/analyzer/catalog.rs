//! Compile a [`LabelCatalogParams`] into an
//! [`elide_core::entity::LabelCatalog`].

use std::sync::OnceLock;

use elide_core::entity::{LabelCatalog, LabelRef};
use nvisy_schema::plan::LabelCatalogParams;

/// Compile a [`LabelCatalogParams`] into an
/// [`elide_core::entity::LabelCatalog`].
///
/// Extension trait kept on the engine side so the cached
/// `with_builtins` lookup and the warn-and-skip policy for unknown
/// builtin names stay out of `nvisy-core`.
pub(crate) trait LabelCatalogCompile {
    /// Build the per-request catalog. Engine does not pre-seed
    /// builtins; the caller picks. Two sources union into one:
    ///
    /// - [`builtins`](LabelCatalogParams::builtins) — each name is
    ///   looked up against the cached full builtin catalog;
    ///   unknown names warn and are skipped (typos shouldn't fail
    ///   the request).
    /// - [`custom`](LabelCatalogParams::custom) — inserted as-is;
    ///   names that collide with a builtin replace it (matches
    ///   [`LabelCatalog::insert`] semantics: last write wins).
    ///
    /// [`LabelCatalog::insert`]: elide_core::entity::LabelCatalog::insert
    fn compile(&self) -> LabelCatalog;
}

impl LabelCatalogCompile for LabelCatalogParams {
    fn compile(&self) -> LabelCatalog {
        let mut catalog = LabelCatalog::new();
        let builtins = builtin_catalog();
        for name in &self.builtins {
            let label_ref = LabelRef::new(name.clone());
            match builtins.get(&label_ref) {
                Some(label) => {
                    catalog.insert(label.clone());
                }
                None => {
                    tracing::warn!(
                        target: "engine::analyzer",
                        label = %name,
                        "unknown builtin label name in catalog request; skipping",
                    );
                }
            }
        }
        for label in &self.custom {
            catalog.insert(label.clone());
        }
        catalog
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
