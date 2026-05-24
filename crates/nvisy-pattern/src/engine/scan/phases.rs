//! Per-phase scan primitives used by [`PatternEngine::scan_entities`].
//!
//! Four phases, each appending [`RawMatch`]es into the shared result
//! buffer:
//!
//! 1. [`scan_regex`] — `RegexSet`-filtered regex candidates.
//! 2. [`scan_glob`] — per-token `GlobSet` candidates.
//! 3. [`scan_dict`] — dictionary Aho-Corasick matches.
//! 4. [`scan_deny_list`] — forced detection of known sensitive values.
//!
//! [`PatternEngine::scan_entities`]: super::super::PatternEngine::scan_entities

use std::collections::HashSet;

use nvisy_ontology::entity::RecognitionMethod;

use super::entries::{CompiledGlobs, DictEntry, GlobBucket, RegexEntry};
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

/// Phase 2: glob matches. Tokenizes `text` on whitespace, trims
/// surrounding ASCII punctuation from each token, then matches every
/// token against the shared [`GlobSet`] in one batched call.
/// `GlobSet::matches` returns the indices of the per-token-matching
/// globs; each index maps back into `glob_entries` for the metadata.
///
/// **Span policy**: the emitted span covers the *trimmed* token —
/// surrounding punctuation that was stripped before matching is
/// excluded. Source `"INV-123,"` matched by glob `INV-*` reports the
/// span `0..7` (the comma at byte 7 is outside).
///
/// **Tokenization limits**: the splitter only knows about whitespace
/// and a small ASCII punctuation set; scripts that don't separate
/// words with whitespace (Chinese, Japanese, Thai, etc.) produce one
/// large "token" per run. For those, use [`RegexEntry`] with explicit
/// boundaries instead.
///
/// Globs are token-shaped by design — use [`RegexEntry`] for
/// substring or cross-token matches.
pub(in crate::engine) fn scan_glob(
    globs: &CompiledGlobs,
    text: &str,
    allow: &AllowList,
    results: &mut Vec<RawMatch>,
) {
    let has_cs = !globs.case_sensitive.entries.is_empty();
    let has_ci = !globs.case_insensitive.entries.is_empty();
    if !has_cs && !has_ci {
        return;
    }

    for (raw, start, end) in tokenize(text) {
        if allow.contains(raw) {
            continue;
        }
        if has_cs {
            match_bucket(&globs.case_sensitive, raw, raw, start, end, results);
        }
        if has_ci {
            let lowered = raw.to_lowercase();
            match_bucket(&globs.case_insensitive, &lowered, raw, start, end, results);
        }
    }
}

/// Match `candidate` against `bucket`'s shared [`globset::GlobSet`]
/// and push one raw match per hit. `raw` is the original substring
/// used for the entity's `value`; `candidate` may differ when the
/// bucket is case-insensitive (lowercased copy).
fn match_bucket(
    bucket: &GlobBucket,
    candidate: &str,
    raw: &str,
    start: usize,
    end: usize,
    results: &mut Vec<RawMatch>,
) {
    for idx in bucket.set.matches(candidate) {
        let entry = &bucket.entries[idx];
        results.push(RawMatch {
            pattern_name: Some(entry.pattern_name.clone()),
            category: entry.category,
            entity_kind: entry.entity_kind,
            value: raw.to_owned(),
            start,
            end,
            confidence: entry.confidence,
            recognition_methods: smallvec::smallvec![RecognitionMethod::glob(&entry.pattern_name)],
            context: entry.context.clone(),
        });
    }
}

/// Set of ASCII punctuation characters trimmed off token edges
/// before glob matching.
const TOKEN_TRIM_CHARS: &[char] = &[
    '.', ',', ';', ':', '!', '?', '(', ')', '[', ']', '{', '}', '<', '>', '"', '\'', '`',
];

/// Split `text` into token slices yielding `(slice, start, end)` byte
/// offsets into the original string. Tokens are produced by:
///
/// 1. Splitting on whitespace.
/// 2. Trimming leading + trailing ASCII punctuation from each token
///    (see [`TOKEN_TRIM_CHARS`]).
///
/// Empty resulting tokens are dropped. The returned offsets always
/// point at the trimmed substring inside the original string, so
/// downstream span construction can use them verbatim.
fn tokenize(text: &str) -> impl Iterator<Item = (&str, usize, usize)> {
    text.split_whitespace().filter_map(move |word| {
        let offset = word.as_ptr() as usize - text.as_ptr() as usize;
        let trimmed_start = word.trim_start_matches(TOKEN_TRIM_CHARS);
        let lead = word.len() - trimmed_start.len();
        let trimmed = trimmed_start.trim_end_matches(TOKEN_TRIM_CHARS);
        if trimmed.is_empty() {
            return None;
        }
        Some((trimmed, offset + lead, offset + lead + trimmed.len()))
    })
}

/// Phase 3: inject deny-list values found in `text` not already matched
/// by regex or dictionary. Total scan is O(n + matches) via the
/// pre-compiled Aho-Corasick automaton on [`DenyList`].
pub(in crate::engine) fn scan_deny_list(text: &str, deny: &DenyList, results: &mut Vec<RawMatch>) {
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
            recognition_methods: smallvec::smallvec![RecognitionMethod::deny_list()],
            context: None,
        });
    }
}
