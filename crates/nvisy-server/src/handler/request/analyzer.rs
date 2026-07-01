//! Per-request analyzer params: layered overrides on top of the
//! deployment's [`AnalyzerParams`] default.
//!
//! Every override field defaults to "inherit"; clients say only
//! what they want different from the deployment default. The
//! resolution method folds the two into a final
//! [`AnalyzerParams`] handed to the engine. Total — no failure
//! mode; the engine compile validates the resolved spec.
//!
//! ## Override kinds
//!
//! - [`ScalarOverride<T>`] wraps a single-value field. Three
//!   variants: `Inherit` (use default), `Replace { value }`
//!   (set to value), `Remove` (clear an `Option` slot, even if
//!   the default had it set).
//! - [`CollectionOverride<T, S>`] wraps a `Vec<T>` field. Three
//!   variants: `Inherit`, `Replace { values }`, and
//!   `Patch { extend, remove }` — filter the default by removing
//!   matching items via `S` selectors, then append `extend`.
//!
//! ## Wire shape per field
//!
//! - `recognizers` ([`RecognizerOverrides`]): nested struct
//!   carrying one override per kind. `pattern` is at-most-one
//!   (scalar), `ner` is a list with selector-based patch
//!   semantics, `llm` is a boolean toggle (the deployment owns
//!   the lineup, requests only opt in or out).
//! - `enrichers` ([`EnricherOverrides`]): nested struct, each
//!   slot at-most-one (scalar). Slots: `language`, `ocr`, `stt`.
//! - `deduplication`: scalar.
//! - `scope` ([`ScopeOverrides`]): nested struct, four scalars
//!   (`languages`, `countries`, `labels`, `labelCatalog`).

use elide_core::primitive::{CountryCode, Languages};
use nvisy_schema::plan::{
    AnalyzerParams, DeduplicationParams, EnricherParams, LabelCatalogParams,
    LanguageEnricherParams, NerRecognizerParams, OcrEnricherParams, PatternRecognizerParams,
    RecognizerParams, ScopeParams, SttEnricherParams,
};
use schemars::JsonSchema;
use serde::Deserialize;

/// Per-request analyzer params. Every field defaults to
/// [`ScalarOverride::Inherit`] / [`CollectionOverride::Inherit`];
/// the request may set any subset.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerOverrides {
    /// Per-kind recognizer overrides.
    #[serde(default)]
    pub recognizers: RecognizerOverrides,
    /// Per-kind enricher overrides.
    #[serde(default)]
    pub enrichers: EnricherOverrides,
    /// Deduplication pipeline. Scalar — replace or inherit.
    #[serde(default)]
    pub deduplication: ScalarOverride<DeduplicationParams>,
    /// Caller-asserted scope (languages, countries, labels,
    /// label catalog).
    #[serde(default)]
    pub scope: ScopeOverrides,
}

impl AnalyzerOverrides {
    /// Fold the request's overrides into the deployment default
    /// to produce the final params handed to the engine.
    pub fn resolve(self, default: &AnalyzerParams) -> AnalyzerParams {
        AnalyzerParams {
            recognizers: self.recognizers.resolve(&default.recognizers),
            enrichers: self.enrichers.resolve(&default.enrichers),
            deduplication: self.deduplication.resolve(&default.deduplication),
            scope: self.scope.resolve(&default.scope),
        }
    }
}

/// Per-knob overrides on the [`ScopeParams`] slots: each is a
/// scalar (replace or inherit).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeOverrides {
    /// Caller-asserted languages. Scalar — replace or inherit.
    #[serde(default)]
    pub languages: ScalarOverride<Languages>,
    /// Caller-asserted jurisdictions. Scalar — replace or
    /// inherit.
    #[serde(default)]
    pub countries: ScalarOverride<Vec<CountryCode>>,
    /// Document-level classification labels. Scalar — replace or
    /// inherit.
    #[serde(default)]
    pub labels: ScalarOverride<Vec<String>>,
    /// Per-request label catalog (builtins + custom). Scalar —
    /// replace or inherit the deployment default wholesale.
    #[serde(default)]
    pub label_catalog: ScalarOverride<LabelCatalogParams>,
}

impl ScopeOverrides {
    fn resolve(self, default: &ScopeParams) -> ScopeParams {
        ScopeParams {
            languages: self.languages.resolve(&default.languages),
            countries: self.countries.resolve(&default.countries),
            labels: self.labels.resolve(&default.labels),
            label_catalog: self.label_catalog.resolve(&default.label_catalog),
        }
    }
}

/// Per-kind overrides on the recognizer slots of
/// [`RecognizerParams`]. Pattern is scalar (at-most-one); NER
/// is a collection; LLM is a scalar boolean toggle (the
/// deployment owns the lineup — the request only opts in or out).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerOverrides {
    /// Pattern recognizer slot. Scalar.
    #[serde(default)]
    pub pattern: ScalarOverride<PatternRecognizerParams>,
    /// NER recognizer list. Selectors match by `name`.
    #[serde(default)]
    pub ner: CollectionOverride<NerRecognizerParams, NerSelector>,
    /// LLM toggle. `true` attaches the deployment's configured
    /// recognizer lineup; `false` skips LLM entirely.
    #[serde(default)]
    pub llm: ScalarOverride<bool>,
}

impl RecognizerOverrides {
    fn resolve(self, default: &RecognizerParams) -> RecognizerParams {
        RecognizerParams {
            pattern: self.pattern.resolve_optional(default.pattern.as_ref()),
            ner: self.ner.resolve(&default.ner, ner_matches),
            llm: self.llm.resolve(&default.llm),
        }
    }
}

/// Per-kind overrides on the enricher slots of
/// [`EnricherParams`]. Every kind is scalar (at-most-one).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnricherOverrides {
    /// Language enricher slot.
    #[serde(default)]
    pub language: ScalarOverride<LanguageEnricherParams>,
    /// OCR enricher slot (image modality only).
    #[serde(default)]
    pub ocr: ScalarOverride<OcrEnricherParams>,
    /// STT enricher slot (audio modality only).
    #[serde(default)]
    pub stt: ScalarOverride<SttEnricherParams>,
}

impl EnricherOverrides {
    fn resolve(self, default: &EnricherParams) -> EnricherParams {
        EnricherParams {
            language: self.language.resolve_optional(default.language.as_ref()),
            ocr: self.ocr.resolve_optional(default.ocr.as_ref()),
            stt: self.stt.resolve_optional(default.stt.as_ref()),
        }
    }
}

/// Per-field override for a scalar (single-value) field.
///
/// Renamed in the generated schema to `{T}Override` so each
/// monomorphisation gets a distinct, descriptive schema name
/// (`DeduplicationParamsOverride`, `ScopeOverride`, …)
/// instead of `schemars`'s numeric collision fallback
/// (`ScalarOverride`, `ScalarOverride2`, `ScalarOverride3`, …).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[schemars(rename = "{T}Override")]
pub enum ScalarOverride<T> {
    /// Use the server default's value for this field.
    #[default]
    Inherit,
    /// Replace the server default's value entirely.
    Replace {
        /// The value to use.
        value: T,
    },
    /// Clear an `Option` slot, even if the default had it set.
    /// On non-`Option` slots (`deduplication`, `scope`) `Remove`
    /// is equivalent to `Inherit` — there's nothing to clear.
    Remove,
}

impl<T: Clone> ScalarOverride<T> {
    /// Resolve against a *required* default. `Remove` falls back
    /// to `Inherit` semantics (the slot is not optional, so
    /// clearing it isn't meaningful).
    fn resolve(self, default: &T) -> T {
        match self {
            Self::Inherit | Self::Remove => default.clone(),
            Self::Replace { value } => value,
        }
    }

    /// Resolve against an *optional* slot.
    fn resolve_optional(self, default: Option<&T>) -> Option<T> {
        match self {
            Self::Inherit => default.cloned(),
            Self::Replace { value } => Some(value),
            Self::Remove => None,
        }
    }
}

/// Per-field override for a collection (`Vec<T>`) field.
///
/// Renamed in the generated schema to `{T}CollectionOverride` so
/// each monomorphisation gets a distinct, descriptive schema name
/// (the selector type `S` is always determined by `T`, so it does
/// not need to appear in the name).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[schemars(rename = "{T}CollectionOverride")]
pub enum CollectionOverride<T, S> {
    /// Use the server default's collection.
    #[default]
    Inherit,
    /// Replace the server default's collection entirely.
    Replace {
        /// The values to use.
        values: Vec<T>,
    },
    /// Filter the server default by removing matching entries
    /// (selectors), then append `extend`. Both arrays are
    /// required on the wire (use `[]` for "no items"); the
    /// derive can't supply a default-on-omit without forcing
    /// `Default` bounds onto `T` / `S` we don't want.
    Patch {
        /// Items to add after applying `remove`.
        extend: Vec<T>,
        /// Selectors that drop matching items from the
        /// inherited collection.
        remove: Vec<S>,
    },
}

impl<T: Clone, S> CollectionOverride<T, S> {
    fn resolve(self, default: &[T], matches: impl Fn(&S, &T) -> bool) -> Vec<T> {
        match self {
            Self::Inherit => default.to_vec(),
            Self::Replace { values } => values,
            Self::Patch { extend, remove } => {
                let mut out: Vec<T> = default
                    .iter()
                    .filter(|item| !remove.iter().any(|sel| matches(sel, item)))
                    .cloned()
                    .collect();
                out.extend(extend);
                out
            }
        }
    }
}

/// Selector for [`CollectionOverride::Patch::remove`] on the
/// `ner` list. NER recognizers are keyed by `name`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NerSelector {
    /// NER recognizer name to match.
    pub name: String,
}

fn ner_matches(sel: &NerSelector, spec: &NerRecognizerParams) -> bool {
    sel.name == spec.name
}
