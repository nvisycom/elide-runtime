//! Attach [`elide::recognition::pattern::PatternRecognizer`] to
//! a per-modality [`Analyzer`]. Modality-generic — a single
//! recognizer instance serves every `M: TextRecognizable`.
//!
//! The bare shipped `elide-pattern` set (built-in regex +
//! dictionaries) always attaches; the request supplies any
//! caller-inlined [`CustomPatternRule`] / [`CustomDictionary`]
//! on [`RecognizerParams`]. Wrapped in elide's `Enhanced` layer
//! so per-label context keywords always boost low-confidence
//! matches.
//!
//! Compile cost is bounded by hardcoded guardrails on rule
//! count, dictionary automaton size, and per-regex source
//! length. The regex NFA/DFA size limits are not applied here:
//! elide's single-budget API cannot separate individual regex
//! NFA size from shared `RegexSet` union size, and the two want
//! very different bounds — the shipped builtins union past any
//! budget that would still catch a runaway custom rule.

use std::collections::HashMap;

use elide::detection::Analyzer;
use elide::recognition::context::Enhanced;
use elide::recognition::pattern::{
    Context as PatternContext, Dictionary, PatternRecognizer, PatternRecognizerBuilder, Regex,
    Scoring, Term, Variant,
};
use elide_core::modality::TextRecognizable;
use elide_core::primitive::LanguageTag;
use elide_core::recognition::Recognizer;
use elide_core::{Error, ErrorKind, Result};
use nvisy_schema::plan::{
    CustomDictionary, CustomDictionaryTerm, CustomPatternContext, CustomPatternRule,
    CustomPatternVariant, MAX_REGEX_SOURCE_LEN, RecognizerParams,
};

/// Maximum number of caller-inlined rules per request across
/// `custom` and `custom_dictionaries` combined.
const MAX_CUSTOM_RULES: usize = 32;

/// Aggregate cap on total dictionary terms across every
/// dictionary — builtin and custom — compiled into one shared
/// Aho-Corasick automaton.
const MAX_DICTIONARY_TERM_COUNT: usize = 100_000;

/// Aggregate byte budget across every dictionary's terms.
const MAX_DICTIONARY_TERM_BYTES: usize = 8 * 1024 * 1024;

/// Attach a [`PatternRecognizer`] built from `spec`.
///
/// The bare shipped built-in pattern + dictionary sets always
/// load; caller-inlined `custom` / `custom_dictionaries` fold in
/// alongside. The recognizer is always wrapped in elide's
/// `Enhanced` layer so per-label context keywords boost
/// low-confidence matches.
pub(in crate::analyzer) fn attach<M>(
    analyzer: Analyzer<M>,
    spec: &RecognizerParams,
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
{
    check_custom_count(spec)?;

    let mut builder = with_limits(PatternRecognizer::builder())
        .with_builtin_patterns()
        .with_builtin_dictionaries();
    for rule in &spec.custom {
        builder = builder.with_pattern(compile_custom_rule(rule)?);
    }
    for dict in &spec.custom_dictionaries {
        builder = builder.with_dictionary(compile_custom_dictionary(dict)?);
    }
    Ok(analyzer.with_recognizer(builder.build_context_enhanced()?))
}

/// Apply the dictionary-size limits to the pattern recognizer
/// builder.
fn with_limits(builder: PatternRecognizerBuilder) -> PatternRecognizerBuilder {
    builder
        .with_term_count_limit(MAX_DICTIONARY_TERM_COUNT)
        .with_term_bytes_limit(MAX_DICTIONARY_TERM_BYTES)
}

fn check_custom_count(spec: &RecognizerParams) -> Result<()> {
    let total = spec.custom.len() + spec.custom_dictionaries.len();
    if total > MAX_CUSTOM_RULES {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "pattern recognizer: {} custom rules requested \
                 (custom={} + customDictionaries={}) exceeds the \
                 per-request cap of {}",
                total,
                spec.custom.len(),
                spec.custom_dictionaries.len(),
                MAX_CUSTOM_RULES,
            ),
        ));
    }
    Ok(())
}

fn compile_custom_rule(rule: &CustomPatternRule) -> Result<Regex> {
    let variants = rule
        .variants
        .iter()
        .map(compile_custom_variant)
        .collect::<Result<Vec<_>, _>>()?;
    Regex::builder()
        .with_name(rule.name.clone())
        .with_labels(vec![rule.label.clone()])
        .with_variants(variants)
        .with_context(compile_context(&rule.context))
        .with_languages(rule.languages.clone())
        .with_countries(rule.countries.clone())
        .build()
}

fn compile_custom_variant(variant: &CustomPatternVariant) -> Result<Variant> {
    if variant.regex.len() > MAX_REGEX_SOURCE_LEN {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "pattern recognizer: regex source of {} bytes exceeds \
                 the cap of {} bytes",
                variant.regex.len(),
                MAX_REGEX_SOURCE_LEN,
            ),
        ));
    }
    let mut out = Variant::new(variant.regex.clone())?.with_score(variant.score);
    if let Some(validator) = &variant.validator {
        out = out.with_validator(validator.clone());
    }
    Ok(out)
}

fn compile_custom_dictionary(dict: &CustomDictionary) -> Result<Dictionary> {
    let terms = dict
        .terms
        .iter()
        .map(compile_custom_term)
        .collect::<Vec<_>>();
    Dictionary::builder()
        .with_name(dict.name.clone())
        .with_labels(vec![dict.label.clone()])
        .with_terms(terms)
        .with_scoring(Scoring::Uniform(dict.score))
        .with_context(compile_context(&dict.context))
        .with_languages(dict.languages.clone())
        .with_countries(dict.countries.clone())
        .build()
}

fn compile_custom_term(term: &CustomDictionaryTerm) -> Term {
    let mut out = Term::new(term.term.clone());
    if let Some(score) = term.score {
        out = out.with_score(score);
    }
    out
}

fn compile_context(context: &CustomPatternContext) -> PatternContext {
    match context {
        CustomPatternContext::Global(kws) => PatternContext::Global(kws.clone()),
        CustomPatternContext::PerLanguage(map) => {
            let converted: HashMap<LanguageTag, Vec<String>> = map
                .iter()
                .map(|(lang, kws)| (lang.clone(), kws.clone()))
                .collect();
            PatternContext::PerLanguage(converted)
        }
    }
}
