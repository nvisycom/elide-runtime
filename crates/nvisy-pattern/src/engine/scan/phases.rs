//! Per-phase scan primitives used by [`PatternEngine::scan_entities`].
//!
//! Three phases, each appending [`EntityCandidate`]s into the shared
//! result buffer:
//!
//! 1. [`scan_regex`] — `RegexSet`-filtered regex candidates.
//! 2. [`scan_dict`] — dictionary Aho-Corasick matches.
//! 3. [`scan_deny_list`] — forced detection of known sensitive values.
//!
//! [`PatternEngine::scan_entities`]: super::super::PatternEngine::scan_entities

use nvisy_ontology::entity::{Entity, RecognitionMethod};
use nvisy_ontology::primitive::Confidence;

use super::candidate::EntityCandidate;
use super::entries::{DictEntry, RegexEntry};
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
    results: &mut Vec<EntityCandidate>,
) {
    for idx in candidates {
        let entry = &regex_entries[idx];

        for mat in entry.regex.find_iter(text) {
            let value = mat.as_str();

            if allow.contains(value) {
                continue;
            }

            // Unknown validator names fall through (match is kept) —
            // the load-time `JsonPatternWarning::UnknownValidator`
            // already signals the typo, so degrading gracefully here
            // beats silently dropping every match.
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

            results.push(EntityCandidate::new(
                Entity::for_text_span(
                    entry.category,
                    entry.entity_kind,
                    vec![method],
                    Confidence::clamped(entry.confidence),
                    mat.start(),
                    mat.end(),
                ),
                entry.context.clone(),
            ));
        }
    }
}

/// Phase 2: dictionary matches via Aho-Corasick automata.
pub(in crate::engine) fn scan_dict(
    dict_entries: &[DictEntry],
    text: &str,
    allow: &AllowList,
    results: &mut Vec<EntityCandidate>,
) {
    for entry in dict_entries {
        for mat in entry.automaton.find_iter(text) {
            let pat_idx = mat.pattern().as_usize();
            let term = &entry.terms[pat_idx];

            if allow.contains(term.value.as_str()) {
                continue;
            }

            results.push(EntityCandidate::new(
                Entity::for_text_span(
                    entry.category,
                    entry.entity_kind,
                    vec![RecognitionMethod::dictionary(&entry.pattern_name)],
                    Confidence::clamped(entry.resolve_confidence(pat_idx)),
                    mat.start(),
                    mat.end(),
                ),
                entry.context.clone(),
            ));
        }
    }
}

/// Phase 3: inject deny-list values found in `text` not already
/// matched by regex / dictionary. Total scan is O(n + matches) via
/// the pre-compiled Aho-Corasick automaton on [`DenyList`].
///
/// Suppresses any deny-list value whose matched substring already
/// appears as a candidate at the same span — comparison is on the
/// candidate's `Location` slice in `text`.
pub(in crate::engine) fn scan_deny_list(
    text: &str,
    deny: &DenyList,
    results: &mut Vec<EntityCandidate>,
) {
    let Some(scanner) = deny.scanner() else {
        return;
    };

    // Collect substrings already matched by earlier phases so we can
    // suppress a deny-list injection that would just re-name what's
    // already been detected.
    let already_matched: std::collections::HashSet<&str> = results
        .iter()
        .filter_map(|c| c.entity.location.as_text())
        .map(|t| &text[t.start_offset..t.end_offset])
        .collect();

    for mat in scanner.automaton.find_iter(text) {
        let (value, rule) = &scanner.entries[mat.pattern().as_usize()];
        if already_matched.contains(value.as_str()) {
            continue;
        }

        results.push(EntityCandidate::new(
            Entity::for_text_span(
                rule.category,
                rule.entity_kind,
                vec![RecognitionMethod::deny_list()],
                Confidence::clamped(1.0),
                mat.start(),
                mat.end(),
            ),
            None,
        ));
    }
}
