//! Compile a [`nvisy_core::plan::ScopeParams`] into an
//! [`elide::recognition::Scope`].

use elide_core::entity::LabelCatalog;
use elide_core::primitive::{CountryCode, Language};
use elide_core::recognition::Scope;
use elide_core::{Error, ErrorKind};
use nvisy_core::plan::ScopeParams;

/// Translate `params` into an [`elide::recognition::Scope`],
/// stamping `catalog` onto it.
///
/// `Scope` is modality-free in elide, so one scope drives every
/// per-modality analyzer the orchestrator routes a document
/// through. Languages flow through as asserted entries;
/// jurisdictions parse into [`CountryCode`]s.
pub(crate) fn compile_scope(params: &ScopeParams, catalog: LabelCatalog) -> Result<Scope, Error> {
    let mut scope = Scope::new().with_catalog(catalog);
    for lang in &params.languages {
        scope = scope.with_language(Language::asserted(lang.clone()));
    }
    for code in &params.jurisdictions {
        let country = CountryCode::from_alpha2(code).map_err(|e| {
            Error::new(
                ErrorKind::Validation,
                format!("scope: jurisdiction `{code}`: {e}"),
            )
        })?;
        scope = scope.with_country(country);
    }
    Ok(scope)
}
