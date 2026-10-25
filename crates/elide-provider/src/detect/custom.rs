//! User-supplied patterns: validating what a request declares,
//! then compiling it into a recognizer.
//!
//! Validation rather than construction — a request's own regexes
//! and term lists are the one detection input this runtime does not
//! take on trust, so the limits and label checks live here.

use std::sync::LazyLock;

use elide::detection::Analyzer;
use elide::entity::LabelCatalog;
use elide::modality::TextRecognizable;
use elide::recognition::Recognizer;
use elide::recognition::pattern::{
    Dictionary, PatternRecognizer, PatternRecognizerBuilder, Regex, RegexBudget, Scoring, Term,
    Variant,
};
use elide::{Error, ErrorKind, Result};
use elide_schema::recognition::{CustomMatcher, MatchOn, Recognition};

use super::pattern::{
    MAX_CUSTOM_MATCHERS, MAX_CUSTOM_REGEX_BYTES, MAX_CUSTOM_TERMS, pattern_with_limits,
};

pub(super) fn attach_custom<M>(
    analyzer: Analyzer<M>,
    recognition: &[Recognition],
) -> Result<Analyzer<M>>
where
    M: TextRecognizable,
    PatternRecognizer: Recognizer<M> + 'static,
{
    let total: usize = recognition.iter().map(|r| r.matchers.len()).sum();
    if total == 0 {
        return Ok(analyzer);
    }
    if total > MAX_CUSTOM_MATCHERS {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "this request declares {total} custom matchers, over the \
                 limit of {MAX_CUSTOM_MATCHERS}: each compiles on the request \
                 path before detection starts.",
            ),
        ));
    }

    // Per-variant, not the union: this recognizer holds only
    // caller-supplied patterns, so the budget that matters is what
    // one of them may compile to. The union budget stays at the
    // crate default — a request's own patterns do not union into
    // anything like the shipped set, and capping it here would
    // bound a total nobody set out to bound.
    //
    // NFA only. The cap exists to reject a pathological source at
    // build; the lazy-DFA cache is a match-time working set, and
    // throttling it would slow matching rather than refuse
    // anything. The two shared a value before elide split them.
    let mut builder = pattern_with_limits(PatternRecognizer::builder())
        .with_variant_budget(RegexBudget::nfa(MAX_CUSTOM_REGEX_BYTES));
    for vocabulary in recognition {
        for matcher in &vocabulary.matchers {
            builder = extend(builder, recognition, matcher)?;
        }
    }
    // Not context-enhanced: boosting reads keyword lists a matcher
    // does not carry, so there is nothing for it to work from.
    Ok(analyzer.with_recognizer(builder.build()?))
}

/// Validate one matcher against its policy and fold it into
/// `builder`.
fn extend(
    builder: PatternRecognizerBuilder,
    recognition: &[Recognition],
    matcher: &CustomMatcher,
) -> Result<PatternRecognizerBuilder> {
    check_label(recognition, matcher)?;
    match &matcher.match_on {
        MatchOn::Pattern { pattern } => {
            let variant = Variant::new(pattern.clone())
                .map_err(|err| invalid(matcher, format!("its regex is invalid: {err}")))?
                .with_score(matcher.confidence);
            let regex = Regex::builder()
                .with_name(matcher.name.clone())
                .with_labels(vec![matcher.label.clone()])
                .with_variants(vec![variant])
                .build()
                .map_err(|err| invalid(matcher, format!("it does not build: {err}")))?;
            Ok(builder.with_pattern(regex))
        }
        MatchOn::Terms { terms } => {
            if terms.is_empty() {
                return Err(invalid(matcher, "its term list is empty".to_owned()));
            }
            if terms.len() > MAX_CUSTOM_TERMS {
                return Err(invalid(
                    matcher,
                    format!(
                        "it lists {} terms, over the limit of {MAX_CUSTOM_TERMS}",
                        terms.len(),
                    ),
                ));
            }
            let dictionary = Dictionary::builder()
                .with_name(matcher.name.clone())
                .with_labels(vec![matcher.label.clone()])
                .with_terms(terms.iter().map(Term::new).collect::<Vec<_>>())
                .with_scoring(Scoring::Uniform(matcher.confidence))
                .build()
                .map_err(|err| invalid(matcher, format!("it does not build: {err}")))?;
            Ok(builder.with_dictionary(dictionary))
        }
    }
}

/// A matcher may only detect a label the request introduces.
///
/// Naming a label the request does not declare would detect into a
/// vocabulary the catalog never held; naming a shipped built-in
/// would race elide's own definition, and reconciliation would
/// pick a winner by confidence rather than by intent.
fn check_label(recognition: &[Recognition], matcher: &CustomMatcher) -> Result<()> {
    if BUILTIN_LABELS.contains(&matcher.label) {
        return Err(invalid(
            matcher,
            format!(
                "`{}` is a label elide already detects: a matcher for it would \
                 race the shipped definition. Introduce a label of your own.",
                matcher.label.as_str(),
            ),
        ));
    }
    // Any of the request's vocabularies may declare it: they merge
    // into one catalog, so which one carries the label is moot.
    if recognition
        .iter()
        .flat_map(|r| &r.custom)
        .any(|l| l.to_ref() == matcher.label)
    {
        return Ok(());
    }
    Err(invalid(
        matcher,
        format!(
            "`{}` is not among the request's own `custom` labels: a matcher \
             detects a label the request introduces, so declare it there first.",
            matcher.label.as_str(),
        ),
    ))
}

/// Every label elide ships, resolved once.
static BUILTIN_LABELS: LazyLock<LabelCatalog> = LazyLock::new(LabelCatalog::with_builtins);

/// A rejection naming the matcher, so a caller knows which one.
fn invalid(matcher: &CustomMatcher, why: String) -> Error {
    Error::new(
        ErrorKind::Configuration,
        format!("custom matcher `{}` is rejected: {why}", matcher.name),
    )
}
