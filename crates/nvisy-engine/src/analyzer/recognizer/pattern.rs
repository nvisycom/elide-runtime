//! Attach [`elide::recognition::pattern::PatternRecognizer`] to
//! a per-modality [`Analyzer`]. Modality-generic — a single
//! recognizer instance (bare or `Enhanced`-wrapped) serves every
//! `M: TextRecognizable`.
//!
//! Wire specs the compile path consumes:
//!
//! - [`PatternRecognizerParams`] — the top-level toggle: builtins,
//!   context enhancement, and the two caller-inlined slots.
//! - [`CustomPatternRule`] / [`CustomPatternVariant`] — inline
//!   regex rules; converted one field at a time to
//!   [`elide_pattern::Regex`] / [`Variant`].
//! - [`CustomDictionary`] / [`CustomDictionaryTerm`] — inline
//!   literal-term rules; converted to
//!   [`elide_pattern::Dictionary`] / [`Term`].
//!
//! [`PatternGuardrails`] bounds runaway compile cost from any of
//! those slots. See [`guardrails`](super::guardrails) for the
//! per-request budget shape.

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
use elide_core::{Error, ErrorKind};
use nvisy_schema::plan::{
    CustomDictionary, CustomDictionaryTerm, CustomPatternContext, CustomPatternRule,
    CustomPatternVariant, PatternRecognizerParams,
};

use super::PatternGuardrails;

/// Attach a [`PatternRecognizer`] built from `spec`.
///
/// The same recognizer instance — bare or wrapped in elide's
/// `Enhanced` layer — serves any `M: TextRecognizable`, so the
/// helper is uniform across modalities.
///
/// `guardrails` bounds runaway compile cost: rule count,
/// dictionary automaton size, and (below the wire ceiling)
/// per-regex source length.
pub(in crate::analyzer) fn attach<M>(
    analyzer: Analyzer<M>,
    spec: &PatternRecognizerParams,
    guardrails: &PatternGuardrails,
) -> Result<Analyzer<M>, Error>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
    Enhanced<PatternRecognizer>: Recognizer<M> + 'static,
{
    check_custom_count(spec, guardrails)?;

    let mut builder = with_limits(PatternRecognizer::builder(), guardrails);
    if spec.builtins {
        builder = builder.with_builtin_patterns().with_builtin_dictionaries();
    }
    for rule in &spec.custom {
        builder = builder.with_pattern(compile_custom_rule(rule, guardrails)?);
    }
    for dict in &spec.custom_dictionaries {
        builder = builder.with_dictionary(compile_custom_dictionary(dict)?);
    }
    if spec.context_enhanced {
        Ok(analyzer.with_recognizer(builder.build_context_enhanced()?))
    } else {
        Ok(analyzer.with_recognizer(builder.build()?))
    }
}

/// Apply the request-level dictionary-size limits to the pattern
/// recognizer builder.
///
/// Every attach path — with or without builtins, with or without
/// customs — inherits the same budgets.
///
/// The regex NFA/DFA `size_limit` / `dfa_size_limit` knobs are
/// not applied here: elide's single-budget API cannot separate
/// "individual regex NFA size" from "shared `RegexSet` union
/// size", and the two want very different bounds — the shipped
/// builtins union past any budget that would still catch a
/// runaway custom rule. See #317 for the follow-up plan.
fn with_limits(
    builder: PatternRecognizerBuilder,
    guardrails: &PatternGuardrails,
) -> PatternRecognizerBuilder {
    builder
        .with_term_count_limit(guardrails.max_dictionary_term_count)
        .with_term_bytes_limit(guardrails.max_dictionary_term_bytes)
}

fn check_custom_count(
    spec: &PatternRecognizerParams,
    guardrails: &PatternGuardrails,
) -> Result<(), Error> {
    let total = spec.custom.len() + spec.custom_dictionaries.len();
    if total > guardrails.max_custom_rules {
        return Err(Error::new(
            ErrorKind::Validation,
            format!(
                "pattern recognizer: {} custom rules requested \
                 (custom={} + customDictionaries={}) exceeds the \
                 per-request cap of {}",
                total,
                spec.custom.len(),
                spec.custom_dictionaries.len(),
                guardrails.max_custom_rules,
            ),
        ));
    }
    Ok(())
}

fn compile_custom_rule(
    rule: &CustomPatternRule,
    guardrails: &PatternGuardrails,
) -> Result<Regex, Error> {
    let variants = rule
        .variants
        .iter()
        .map(|variant| compile_custom_variant(variant, guardrails))
        .collect::<Result<Vec<_>, _>>()?;
    Regex::builder()
        .with_name(rule.name.clone())
        .with_label(rule.label.clone())
        .with_variants(variants)
        .with_context(compile_context(&rule.context))
        .with_languages(rule.languages.clone())
        .with_countries(rule.countries.clone())
        .build()
}

fn compile_custom_variant(
    variant: &CustomPatternVariant,
    guardrails: &PatternGuardrails,
) -> Result<Variant, Error> {
    if variant.regex.len() > guardrails.max_regex_source_len {
        return Err(Error::new(
            ErrorKind::Validation,
            format!(
                "pattern recognizer: regex source of {} bytes exceeds \
                 the deployment cap of {} bytes",
                variant.regex.len(),
                guardrails.max_regex_source_len,
            ),
        ));
    }
    let mut out = Variant::new(variant.regex.clone())?.with_score(variant.score);
    if let Some(validator) = &variant.validator {
        out = out.with_validator(validator.clone());
    }
    Ok(out)
}

fn compile_custom_dictionary(dict: &CustomDictionary) -> Result<Dictionary, Error> {
    let terms = dict
        .terms
        .iter()
        .map(compile_custom_term)
        .collect::<Vec<_>>();
    Dictionary::builder()
        .with_name(dict.name.clone())
        .with_label(dict.label.clone())
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
