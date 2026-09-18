//! Restricting which registered components a request may run.
//!
//! Two halves, deliberately separate because they answer to
//! different people:
//!
//! - [`Availability`] is what a caller *may* use. It is fixed when
//!   an [`Engine`] is built, by whoever embeds this runtime — not
//!   by the caller. The runtime does not interpret *why* something
//!   is unavailable; a deployment restricting by plan, by data
//!   residency, or by what is still in staging writes the same
//!   thing.
//! - [`Selection`] is what one request *wants*, within that. It
//!   rides [`RequestContext`], and narrowing is all it can do.
//!
//! Asking for something unavailable is refused rather than
//! quietly dropped: a caller that silently got weaker detection
//! than it asked for cannot tell that result from a clean
//! document.
//!
//! [`Engine`]: https://docs.rs/elide-pipeline/latest/elide_pipeline/struct.Engine.html
//! [`RequestContext`]: super::super::RequestContext

use elide::{Error, ErrorKind, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Component, Enrichers, Recognizers};

/// Which registered components a caller may run.
///
/// Set when the engine is built. [`All`](Self::All) is the default,
/// so a deployment that restricts nothing configures nothing.
///
/// Both restricting forms match a component by its name or by any
/// of its [`tags`](Component::tags), so a deployment can name a
/// family once instead of enumerating its members.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    /// Every registered component.
    #[default]
    All,
    /// Only components matching one of these names or tags.
    ///
    /// Closed: a component added to the configuration later is
    /// unavailable until this list names it or one of its tags.
    Only(Vec<String>),
    /// Every component except those matching one of these names or
    /// tags.
    ///
    /// Open: a component added later is available unless this list
    /// excludes it. The form to reach for when new components
    /// should reach existing callers by default.
    Except(Vec<String>),
}

impl Availability {
    /// Whether `component` is available under this restriction.
    fn permits<B>(&self, component: &Component<B>) -> bool {
        match self {
            Self::All => true,
            Self::Only(keys) => matches_any(component, keys),
            Self::Except(keys) => !matches_any(component, keys),
        }
    }
}

/// Whether a component's name or any of its tags is in `keys`.
fn matches_any<B>(component: &Component<B>, keys: &[String]) -> bool {
    keys.iter().any(|key| matches(component, key))
}

/// Whether `key` is this component's name or one of its tags.
fn matches<B>(component: &Component<B>, key: &str) -> bool {
    component.name == key || component.tags.iter().any(|tag| tag == key)
}

/// Which of the available components one request wants to run.
///
/// Empty by default, meaning every component the caller has
/// available. A request that expresses no opinion gets full
/// detection — the opposite would let an omitted field quietly
/// disable redaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct Selection {
    /// Names or tags to run, or [`None`] for all available.
    ///
    /// An empty `Vec` is not the same as [`None`]: it selects
    /// nothing and is refused, because a request that can detect
    /// nothing is a mistake rather than a request for an empty
    /// report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    /// Names or tags to skip, applied after `only`.
    ///
    /// Lets a caller run everything but one component without
    /// enumerating the rest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
}

impl Selection {
    /// A selection running every available component.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same selection, running only these names or tags.
    #[must_use]
    pub fn with_only(mut self, only: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.only = Some(only.into_iter().map(Into::into).collect());
        self
    }

    /// The same selection, skipping these names or tags.
    #[must_use]
    pub fn with_skip(mut self, skip: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.skip = skip.into_iter().map(Into::into).collect();
        self
    }

    /// Whether this selection leaves every available component in.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.only.is_none() && self.skip.is_empty()
    }

    /// Every key this selection names, in `only` then `skip` order.
    fn keys(&self) -> impl Iterator<Item = &String> {
        self.only.iter().flatten().chain(self.skip.iter())
    }

    /// Whether `component` is selected.
    fn selects<B>(&self, component: &Component<B>) -> bool {
        let picked = match &self.only {
            None => true,
            Some(keys) => matches_any(component, keys),
        };
        picked && !matches_any(component, &self.skip)
    }
}

/// The two halves applied together: what the caller may run, and
/// what this request wants of it.
///
/// They are held as one because every question worth asking needs
/// both — a component runs only if availability permits it *and*
/// the selection picks it — and because resolving through a single
/// value keeps the two from being passed in the wrong order or
/// consulted separately.
///
/// Borrowed rather than owned: availability lives on the provider
/// for the life of the engine and selection on the request, so a
/// resolver is made per request and discarded.
pub(crate) struct Resolver<'a> {
    /// What this caller may run at all.
    availability: &'a Availability,
    /// What this request wants of it.
    selection: &'a Selection,
}

/// One key a selection named, and what the registered lineups make
/// of it.
///
/// Validation spans every lineup rather than each one alone: a key
/// naming a NER component is legitimate even though the LLM lineup
/// has never heard of it, so "unknown" only means unknown
/// *everywhere*.
#[derive(Default)]
struct KeyStatus {
    /// Whether any registered component matches the key.
    known: bool,
    /// Whether any *available* component matches it.
    permitted: bool,
}

/// A registered lineup, erased so keys can be checked across
/// lineups of different backend types in one pass.
trait Lineup {
    /// Fold this lineup's verdict on `key` into `status`.
    fn observe(&self, status: &mut KeyStatus, key: &str, availability: &Availability);
}

impl<B> Lineup for &[Component<B>] {
    fn observe(&self, status: &mut KeyStatus, key: &str, availability: &Availability) {
        for component in self.iter().filter(|c| matches(c, key)) {
            status.known = true;
            if availability.permits(component) {
                status.permitted = true;
            }
        }
    }
}

impl<'a> Resolver<'a> {
    /// A resolver applying `selection` within `availability`.
    pub(crate) fn new(availability: &'a Availability, selection: &'a Selection) -> Self {
        Self {
            availability,
            selection,
        }
    }

    /// Check every key the selection names against every registered
    /// lineup, refusing before any lineup is filtered.
    ///
    /// Takes both configs rather than a list of lineups: the check
    /// only means anything when it spans all four, and a caller
    /// assembling that list by hand could leave one out and turn a
    /// valid key into a refusal.
    ///
    /// Refuses rather than narrows: a key matching a component this
    /// caller may not use is a permission error, and a key matching
    /// no registered component at all is a typo. Both would
    /// otherwise present as a quietly smaller lineup, which a
    /// caller cannot distinguish from a document that simply had
    /// less in it.
    pub(crate) fn validate_keys(
        &self,
        recognizers: &Recognizers,
        enrichers: &Enrichers,
    ) -> Result<()> {
        let lineups: [&dyn Lineup; 4] = [
            &recognizers.ner.as_slice(),
            &recognizers.llm.as_slice(),
            &enrichers.ocr.as_slice(),
            &enrichers.stt.as_slice(),
        ];
        for key in self.selection.keys() {
            let mut status = KeyStatus::default();
            for lineup in lineups {
                lineup.observe(&mut status, key, self.availability);
            }
            if !status.known {
                return Err(Error::new(
                    ErrorKind::Configuration,
                    format!("request selects `{key}`, which names no registered recognizer or tag"),
                ));
            }
            if !status.permitted {
                return Err(Error::new(
                    ErrorKind::CapabilityUnavailable,
                    format!("request selects `{key}`, which this caller is not permitted to use"),
                ));
            }
        }
        Ok(())
    }

    /// Filter one lineup to the components that will run.
    ///
    /// Assumes [`validate_keys`](Self::validate_keys) has already
    /// passed, so an empty result here means this lineup
    /// contributed nothing — not that the request was malformed.
    pub(crate) fn resolve<'c, B>(&self, components: &'c [Component<B>]) -> Vec<&'c Component<B>> {
        components
            .iter()
            .filter(|c| self.availability.permits(c) && self.selection.selects(c))
            .collect()
    }

    /// The single enricher this request runs from `lineup`, or
    /// [`None`] for none.
    ///
    /// A deployment may offer several — three OCR engines with
    /// different strengths, say — but elide attaches at most one
    /// per analyzer, so the cap is on what a request *resolves to*
    /// rather than on what is configured. Resolving to none is
    /// legitimate: an absent enricher means the modality is not
    /// enriched, unlike an empty recognizer set, which means
    /// nothing can be detected at all. Resolving to several is
    /// ambiguous, and `kind` names the lineup in that error.
    pub(crate) fn pick_one<'c, B>(
        &self,
        lineup: &'c [Component<B>],
        kind: &str,
    ) -> Result<Option<&'c Component<B>>> {
        match self.resolve(lineup).as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(one)),
            many => Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "{kind} selection resolves to {} enrichers; elide attaches at most one \
                     per analyzer. Narrow the request's selection to one of: {}",
                    many.len(),
                    many.iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            )),
        }
    }

    /// The names one lineup resolves to, for a caller inspecting
    /// what would run rather than building an analyzer.
    pub(crate) fn resolve_names<B>(&self, components: &[Component<B>]) -> Vec<String> {
        self.resolve(components)
            .into_iter()
            .map(|c| c.name.to_string())
            .collect()
    }

    /// Refuse a selection that narrowed every registered recognizer
    /// away.
    ///
    /// A request that can detect nothing is a mistake rather than a
    /// request for an empty report — the same reading as an empty
    /// label catalog. Left to itself it would return a confidently
    /// clean audit for a document full of sensitive data.
    ///
    /// A deployment that wired no recognizer at all is not
    /// selecting them away, so it is left alone: only a lineup that
    /// had something to lose can lose it.
    pub(crate) fn refuse_if_no_recognizer(
        &self,
        recognizers: &Recognizers,
        resolved: usize,
    ) -> Result<()> {
        let registered = recognizers.ner.len() + recognizers.llm.len();
        if resolved == 0 && registered > 0 {
            return Err(Error::new(
                ErrorKind::Configuration,
                "request selects no recognizer: every registered component was either \
                 unavailable or skipped"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// What a [`Selection`] resolves to, lineup by lineup.
///
/// The same resolution a request performs, so a caller can show
/// what a selection would run — or discover what it may run at all
/// — without analyzing a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ResolvedComponents {
    /// The NER recognizers that would run.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ner: Vec<String>,
    /// The LLM recognizers that would run.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub llm: Vec<String>,
    /// The OCR enricher that would run. At most one survives
    /// compile; more than one here is a selection the request must
    /// narrow.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ocr: Vec<String>,
    /// The STT enricher that would run, under the same cap as
    /// [`ocr`](Self::ocr).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stt: Vec<String>,
}
