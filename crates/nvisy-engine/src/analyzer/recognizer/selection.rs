//! Resolve a [`ProviderSelection`] against a deployment-configured
//! lineup, returning the borrowed subset the analyzer should attach.
//!
//! Shared by [`super::ner`] and [`super::llm`]; validation errors
//! (empty allowlist, unknown recognizer name, opt-in with an empty
//! deployment lineup) surface here so both call sites report them
//! the same way.
//!
//! [`ProviderSelection`]: nvisy_schema::plan::ProviderSelection

use std::collections::HashSet;

use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::plan::ProviderSelection;

use crate::provider::{LlmRecognizer, NerRecognizer};

/// One thing every allowlist-selectable recognizer needs: a name.
pub(super) trait Named {
    fn name(&self) -> &str;
}

impl Named for NerRecognizer {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for LlmRecognizer {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Filter `lineup` by `selection`, returning the recognizers to
/// attach in configuration order.
///
/// `field` is the recognizer-kind field name on
/// `RecognizerParams` (`"ner"` / `"llm"`), interpolated verbatim
/// into validation error messages.
pub(super) fn select<'a, T: Named>(
    selection: Option<&ProviderSelection>,
    lineup: &'a [T],
    field: &'static str,
) -> Result<Vec<&'a T>> {
    match selection {
        None => Ok(lineup.iter().collect()),
        Some(ProviderSelection::All(false)) => Ok(Vec::new()),
        Some(ProviderSelection::All(true)) => {
            if lineup.is_empty() {
                return Err(empty_lineup(field));
            }
            Ok(lineup.iter().collect())
        }
        Some(ProviderSelection::Only(names)) => select_allowlisted(lineup, names, field),
    }
}

fn select_allowlisted<'a, T: Named>(
    lineup: &'a [T],
    names: &[String],
    field: &'static str,
) -> Result<Vec<&'a T>> {
    if names.is_empty() {
        return Err(empty_allowlist(field));
    }
    let available: HashSet<&str> = lineup.iter().map(Named::name).collect();
    let unknown: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| !available.contains(n))
        .collect();
    if !unknown.is_empty() {
        return Err(unknown_names(field, &unknown, &available));
    }
    let allowed: HashSet<&str> = names.iter().map(String::as_str).collect();
    Ok(lineup
        .iter()
        .filter(|r| allowed.contains(r.name()))
        .collect())
}

fn empty_lineup(field: &str) -> Error {
    Error::new(
        ErrorKind::Configuration,
        format!(
            "AnalyzerParams.recognizers.{field} = true but the deployment has no \
             {field} recognizer configured; leave `{field}` unset / false to opt out"
        ),
    )
}

fn empty_allowlist(field: &str) -> Error {
    Error::new(
        ErrorKind::Configuration,
        format!(
            "AnalyzerParams.recognizers.{field} is an empty allowlist; use \
             `{field}: false` to opt out entirely, or list at least one recognizer name"
        ),
    )
}

fn unknown_names(field: &str, unknown: &[&str], available: &HashSet<&str>) -> Error {
    let mut available: Vec<&str> = available.iter().copied().collect();
    available.sort_unstable();
    Error::new(
        ErrorKind::Configuration,
        format!(
            "AnalyzerParams.recognizers.{field} names unknown recognizer(s) {unknown:?}; \
             available in this deployment: {available:?}"
        ),
    )
}
