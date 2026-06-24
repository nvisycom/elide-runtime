//! Compile a [`nvisy_core::plan::ScopeSpec`] into an
//! [`elide::recognition::Scope`].

use elide_core::Error;
use elide_core::modality::Modality;
use elide_core::primitive::{CountryCode, Language, LanguageTag};
use elide_core::recognition::Scope;
use nvisy_core::plan::ScopeSpec;

/// Translate `spec` into an [`elide::recognition::Scope<M>`].
///
/// Languages are turned into asserted [`Language`] entries;
/// jurisdictions parse into [`CountryCode`]s. Returns the first
/// parse error encountered.
pub(super) fn compile_scope<M: Modality>(spec: &ScopeSpec) -> Result<Scope<M>, Error> {
    let mut scope = Scope::<M>::new();
    for lang in &spec.languages {
        let tag = LanguageTag::try_from(lang.clone())?;
        scope = scope.with_language(Language::asserted(tag));
    }
    for code in &spec.jurisdictions {
        let country = CountryCode::from_alpha2(code).map_err(|e| {
            Error::new(
                elide_core::ErrorKind::Validation,
                format!("scope: jurisdiction `{code}`: {e}"),
            )
        })?;
        scope = scope.with_country(country);
    }
    Ok(scope)
}
