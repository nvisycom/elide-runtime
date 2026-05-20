//! Per-phase scan primitives used by [`PatternEngine::scan_entities`].
//!
//! Three phases, each appending [`RawMatch`]es into the shared result
//! buffer:
//!
//! 1. [`scan_regex`] — `RegexSet`-filtered regex candidates.
//! 2. [`scan_dict`] — dictionary Aho-Corasick matches.
//! 3. [`scan_deny_list`] — forced detection of known sensitive values.
//!
//! [`PatternEngine::scan_entities`]: super::super::PatternEngine::scan_entities

use std::collections::HashSet;

use nvisy_ontology::entity::RecognitionMethod;

use super::entries::{DictEntry, RegexEntry};
use super::pattern_match::RawMatch;
use crate::engine::filter::{AllowList, DenyList};
use crate::validators::ValidatorResolver;

/// Phase 1: regex matches. Uses `RegexSet`-filtered candidates already
/// resolved by the caller; runs each matching regex individually to
/// extract offsets and values.
pub(in crate::engine) fn scan_regex(
    candidates: impl IntoIterator<Item = usize>,
    regex_entries: &[RegexEntry],
    validators: &ValidatorResolver,
    text: &str,
    allow: &AllowList,
    results: &mut Vec<RawMatch>,
) {
    for idx in candidates {
        let entry = &regex_entries[idx];

        for mat in entry.regex.find_iter(text) {
            let value = mat.as_str();

            if allow.contains(value) {
                continue;
            }

            if let Some(ref vname) = entry.validator_name
                && let Some(validate) = validators.resolve(vname)
                && !validate(value)
            {
                continue;
            }

            let method = if let Some(ref vname) = entry.validator_name {
                RecognitionMethod::regex_validated(&entry.pattern_name, vname)
            } else {
                RecognitionMethod::regex(&entry.pattern_name)
            };

            results.push(RawMatch {
                pattern_name: Some(entry.pattern_name.clone()),
                category: entry.category,
                entity_kind: entry.entity_kind,
                value: value.to_owned(),
                start: mat.start(),
                end: mat.end(),
                confidence: entry.confidence,
                recognition_methods: smallvec::smallvec![method],
                context: entry.context.clone(),
            });
        }
    }
}

/// Phase 2: dictionary matches via Aho-Corasick automata.
pub(in crate::engine) fn scan_dict(
    dict_entries: &[DictEntry],
    text: &str,
    allow: &AllowList,
    results: &mut Vec<RawMatch>,
) {
    for entry in dict_entries {
        for mat in entry.automaton.find_iter(text) {
            let pat_idx = mat.pattern().as_usize();
            let term = &entry.terms[pat_idx];

            if allow.contains(term.value.as_str()) {
                continue;
            }

            results.push(RawMatch {
                pattern_name: Some(entry.pattern_name.clone()),
                category: entry.category,
                entity_kind: entry.entity_kind,
                value: term.value.clone(),
                start: mat.start(),
                end: mat.end(),
                confidence: entry.resolve_confidence(pat_idx),
                recognition_methods: smallvec::smallvec![RecognitionMethod::dictionary(
                    &entry.pattern_name
                )],
                context: entry.context.clone(),
            });
        }
    }
}

/// Phase 3: inject deny-list values found in `text` not already matched
/// by regex or dictionary. Total scan is O(n + matches) via the
/// pre-compiled Aho-Corasick automaton on [`DenyList`].
pub(in crate::engine) fn scan_deny_list(
    text: &str,
    deny: &DenyList,
    results: &mut Vec<RawMatch>,
) {
    let Some(scanner) = deny.scanner() else {
        return;
    };

    let matched_values: HashSet<String> = results.iter().map(|r| r.value.clone()).collect();

    for mat in scanner.automaton.find_iter(text) {
        let (value, rule) = &scanner.entries[mat.pattern().as_usize()];
        if matched_values.contains(value) {
            continue;
        }
        results.push(RawMatch {
            pattern_name: None,
            category: rule.category,
            entity_kind: rule.entity_kind,
            value: value.clone(),
            start: mat.start(),
            end: mat.end(),
            confidence: 1.0,
            recognition_methods: smallvec::smallvec![rule.method.clone()],
            context: None,
        });
    }
}
