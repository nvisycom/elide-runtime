//! Resolve a [`ProviderSelection`] against a deployment-configured
//! lineup, returning the borrowed subset the analyzer should attach.
//!
//! Shared by [`super::ner`] and [`super::llm`]; validation errors
//! (empty allowlist, unknown recognizer name, opt-in with an empty
//! deployment lineup) surface here so both call sites report them
//! the same way.
//!
//! [`ProviderSelection`]: nvisy_schema::plan::ProviderSelection

use elide_core::{Error, ErrorKind};
use nvisy_schema::plan::ProviderSelection;

/// Filter `lineup` by `selection`, returning the recognizers to
/// attach in configuration order.
///
/// `name_of` extracts a recognizer's name for allowlist matching;
/// `kind` (`"ner"` / `"llm"`) and `toml_path`
/// (`"[[ner.recognizers]]"` / `"[[llm.recognizers]]"`) shape the
/// error messages.
pub(super) fn select<'a, T, F>(
    selection: Option<&ProviderSelection>,
    lineup: &'a [T],
    name_of: F,
    kind: &'static str,
    toml_path: &'static str,
) -> Result<Vec<&'a T>, Error>
where
    F: Fn(&T) -> &str,
{
    match selection {
        None => Ok(lineup.iter().collect()),
        Some(ProviderSelection::All(false)) => Ok(Vec::new()),
        Some(ProviderSelection::All(true)) => {
            if lineup.is_empty() {
                return Err(Error::new(
                    ErrorKind::Validation,
                    format!(
                        "AnalyzerParams.recognizers.{kind} = true but the deployment has no \
                         {} recognizer configured; add one to `{toml_path}` in the deployment \
                         config or leave `{kind}` unset / false",
                        kind.to_ascii_uppercase(),
                    ),
                ));
            }
            Ok(lineup.iter().collect())
        }
        Some(ProviderSelection::Only(names)) => {
            if names.is_empty() {
                return Err(Error::new(
                    ErrorKind::Validation,
                    format!(
                        "AnalyzerParams.recognizers.{kind} is an empty allowlist; use \
                         `{kind}: false` to opt out entirely, or list at least one recognizer name"
                    ),
                ));
            }
            let unknown: Vec<&str> = names
                .iter()
                .filter(|n| !lineup.iter().any(|r| name_of(r) == n.as_str()))
                .map(String::as_str)
                .collect();
            if !unknown.is_empty() {
                let available: Vec<&str> = lineup.iter().map(&name_of).collect();
                return Err(Error::new(
                    ErrorKind::Validation,
                    format!(
                        "AnalyzerParams.recognizers.{kind} names unknown recognizer(s) {unknown:?}; \
                         available in this deployment: {available:?}"
                    ),
                ));
            }
            Ok(lineup
                .iter()
                .filter(|r| names.iter().any(|n| n == name_of(r)))
                .collect())
        }
    }
}
