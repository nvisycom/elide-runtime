//! Localization: map an [`NerCandidate`] to a byte range in the
//! source text using its [`context`] hint.
//!
//! Both context and source are normalized (NFC + whitespace
//! collapse) before searching to absorb LLM whitespace drift. Byte
//! offsets returned are in the *original*, un-normalized text.
//!
//! Used by [`NerAgent::detect`] to lift LLM-produced candidates
//! into entity byte ranges before the [`build`] module turns them
//! into entities. Both hint-response candidates (which carry a
//! `hint_id`) and fresh discoveries go through this same path.
//!
//! [`context`]: NerCandidate::context
//! [`NerAgent::detect`]: super::NerAgent::detect
//! [`build`]: super::build

use unicode_normalization::UnicodeNormalization;

use super::NerCandidate;

const TARGET: &str = "nvisy_agent::agent::ner::detect::localize";

/// A candidate that's been resolved to a byte range in the source.
#[derive(Debug, Clone)]
pub(crate) struct LocalizedCandidate {
    pub candidate: NerCandidate,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// What to do with candidates that can't be uniquely localized.
///
/// Both variants log a WARN line per dropped candidate — there's
/// no silent-drop mode because losing a candidate the LLM
/// produced is always operationally interesting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnresolvedCandidatePolicy {
    /// Drop ambiguous and missing candidates. Default.
    #[default]
    Drop,
    /// Pick the first match for ambiguous candidates; drop only
    /// when there are zero matches.
    FirstMatch,
}

/// Localize every candidate against the source text.
///
/// Returns one [`LocalizedCandidate`] per resolvable input. Drops
/// (per policy) those whose context is absent, missing from the
/// source, or ambiguous.
pub(crate) fn localize_all(
    text: &str,
    candidates: Vec<NerCandidate>,
    policy: UnresolvedCandidatePolicy,
) -> Vec<LocalizedCandidate> {
    // Precompute the normalized source + index maps once.
    let (normalized_text, index_maps) = normalize_with_index_map(text);

    let mut out = Vec::with_capacity(candidates.len());
    for c in candidates {
        if let Some(localized) = localize_one(&normalized_text, &index_maps, &c, policy) {
            out.push(localized);
        }
        // Dropped candidates emit a warning from inside localize_one.
    }
    out
}

fn localize_one(
    normalized_text: &str,
    orig_index: &(Vec<usize>, Vec<usize>),
    candidate: &NerCandidate,
    policy: UnresolvedCandidatePolicy,
) -> Option<LocalizedCandidate> {
    let context = match candidate.context.as_deref() {
        Some(c) => c,
        None => {
            warn_dropped(candidate, "no context");
            return None;
        }
    };
    let (normalized_context, _) = normalize_with_index_map(context);
    let (normalized_value, _) = normalize_with_index_map(&candidate.value);

    // Find context matches in normalized text.
    let context_matches: Vec<usize> = normalized_text
        .match_indices(&normalized_context)
        .map(|(i, _)| i)
        .collect();

    let context_start = match context_matches.len() {
        0 => {
            warn_dropped(candidate, "context not found");
            return None;
        }
        1 => context_matches[0],
        _ => match policy {
            UnresolvedCandidatePolicy::FirstMatch => context_matches[0],
            _ => {
                warn_dropped(candidate, "context ambiguous");
                return None;
            }
        },
    };

    // Within the context window, find the value.
    let context_end = context_start + normalized_context.len();
    let window = &normalized_text[context_start..context_end];
    let value_matches: Vec<usize> = window
        .match_indices(&normalized_value)
        .map(|(i, _)| i)
        .collect();
    let value_offset = match value_matches.len() {
        0 => {
            warn_dropped(candidate, "value not found in context");
            return None;
        }
        1 => value_matches[0],
        _ => match policy {
            UnresolvedCandidatePolicy::FirstMatch => value_matches[0],
            _ => {
                warn_dropped(candidate, "value ambiguous within context");
                return None;
            }
        },
    };

    let norm_start = context_start + value_offset;
    let norm_end = norm_start + normalized_value.len();

    // Map back to original byte offsets. start_index gives the
    // original byte where the normalized char *begins*; end_index
    // gives the original byte where the normalized char *ends* (one
    // past). Both maps have a final sentinel so norm_end can equal
    // the normalized text length.
    let (start_index, end_index) = orig_index;
    let start_offset = *start_index.get(norm_start)?;
    // end_index is indexed by the byte *before* norm_end. When
    // norm_end is at the very end of normalized text, fall back to
    // the sentinel (last element of start_index).
    let end_offset = if norm_end == 0 {
        start_offset
    } else {
        *end_index.get(norm_end - 1)?
    };

    Some(LocalizedCandidate {
        candidate: candidate.clone(),
        start_offset,
        end_offset,
    })
}

fn warn_dropped(c: &NerCandidate, reason: &str) {
    tracing::warn!(
        target: TARGET,
        entity_id = ?c.entity_id,
        value = %c.value,
        reason,
        "dropping unresolvable NER candidate"
    );
}

/// Normalize text (NFC + whitespace collapse to single ASCII space)
/// and return:
///
/// 1. The normalized string.
/// 2. `(start_index, end_index)`: parallel maps from normalized
///    byte positions to original byte positions. `start_index[i]`
///    is the original byte offset where the normalized char
///    containing byte `i` *begins*; `end_index[i]` is the original
///    byte offset *one past* the end of that same char.
///
/// Two maps are needed because NFC normalization can change byte
/// widths (NFD precomposed → single NFC char, etc.) so the
/// original char's end offset is not generally
/// `start_index[i] + normalized_char_len`.
///
/// Walking by *original* chars (not normalized chars) keeps the
/// width arithmetic correct: each original char contributes its
/// known `len_utf8` to `end - start` in original space, and its
/// NFC expansion contributes some number of bytes to the
/// normalized output.
fn normalize_with_index_map(text: &str) -> (String, (Vec<usize>, Vec<usize>)) {
    let mut out = String::with_capacity(text.len());
    let mut start_index: Vec<usize> = Vec::with_capacity(text.len());
    let mut end_index: Vec<usize> = Vec::with_capacity(text.len());
    let mut last_was_space = false;
    let mut orig_pos = 0usize;

    for orig_ch in text.chars() {
        let orig_ch_len = orig_ch.len_utf8();
        let orig_end = orig_pos + orig_ch_len;

        if orig_ch.is_whitespace() {
            if !last_was_space {
                // Emit a single space; record this whitespace
                // char's original byte range so anything matching
                // here maps back correctly.
                start_index.push(orig_pos);
                end_index.push(orig_end);
                out.push(' ');
                last_was_space = true;
            }
            // Else: skip this whitespace char entirely in
            // normalized output.
        } else {
            // Apply NFC to just this char (may yield 1+ chars).
            for nfc_ch in orig_ch.to_string().nfc() {
                for _ in 0..nfc_ch.len_utf8() {
                    start_index.push(orig_pos);
                    end_index.push(orig_end);
                }
                out.push(nfc_ch);
            }
            last_was_space = false;
        }
        orig_pos += orig_ch_len;
    }
    // Sentinel for norm_end queries that point one past the end.
    start_index.push(orig_pos);
    end_index.push(orig_pos);
    (out, (start_index, end_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(value: &str, context: Option<&str>) -> NerCandidate {
        NerCandidate {
            entity_id: Some("test_1".into()),
            entity_type: None,
            value: value.into(),
            confidence: None,
            context: context.map(Into::into),
            description: None,
            hint_id: None,
        }
    }

    #[test]
    fn localizes_with_unique_context() {
        let text = "Alice met Bob. Later Alice called him.";
        let c = cand("Alice", Some("Later Alice called"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::default());
        assert_eq!(out.len(), 1);
        let l = &out[0];
        assert_eq!(&text[l.start_offset..l.end_offset], "Alice");
        assert_eq!(l.start_offset, 21);
    }

    #[test]
    fn drops_when_context_missing() {
        let text = "Alice is here.";
        let c = cand("Alice", None);
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::Drop);
        assert!(out.is_empty());
    }

    #[test]
    fn drops_when_context_not_found() {
        let text = "Alice is here.";
        let c = cand("Alice", Some("stale context from another chunk"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::Drop);
        assert!(out.is_empty());
    }

    #[test]
    fn drops_when_context_ambiguous_under_default_policy() {
        let text = "Hi Alice. Hi Alice. Hi Alice.";
        let c = cand("Alice", Some("Hi Alice"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::Drop);
        assert!(out.is_empty());
    }

    #[test]
    fn first_match_policy_picks_first() {
        let text = "Hi Alice. Hi Alice. Hi Alice.";
        let c = cand("Alice", Some("Hi Alice"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::FirstMatch);
        assert_eq!(out.len(), 1);
        assert_eq!(&text[out[0].start_offset..out[0].end_offset], "Alice");
        assert_eq!(out[0].start_offset, 3);
    }

    #[test]
    fn collapses_whitespace_differences() {
        let text = "Hello   world,   John Smith was here.";
        // LLM returns context with single spaces; source has runs.
        let c = cand("John Smith", Some("world, John Smith was"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::default());
        assert_eq!(out.len(), 1);
        assert_eq!(&text[out[0].start_offset..out[0].end_offset], "John Smith");
    }

    #[test]
    fn handles_unicode_nfc() {
        // "café" can be NFC (4 bytes for é = 2 bytes) or NFD
        // (separate combining accent). Both should localize.
        let text = "Hi café here.";
        let c = cand("café", Some("Hi café here"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::default());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn multibyte_end_offset_is_correct() {
        // Multi-byte chars on both sides of the value. If end_offset
        // tracked char *starts*, the slice would end mid-é and panic
        // or return wrong bytes.
        let text = "café — Bob — café";
        let c = cand("Bob", Some("café — Bob — café"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::default());
        assert_eq!(out.len(), 1);
        let l = &out[0];
        // Must slice cleanly without breaking the multibyte chars.
        assert_eq!(&text[l.start_offset..l.end_offset], "Bob");
        // text.is_char_boundary() should hold at both ends.
        assert!(text.is_char_boundary(l.start_offset));
        assert!(text.is_char_boundary(l.end_offset));
    }

    #[test]
    fn multibyte_value_end_offset_is_correct() {
        // Value itself ends with a multi-byte char.
        let text = "Hi café here.";
        let c = cand("café", Some("Hi café here"));
        let out = localize_all(text, vec![c], UnresolvedCandidatePolicy::default());
        assert_eq!(out.len(), 1);
        let l = &out[0];
        assert_eq!(&text[l.start_offset..l.end_offset], "café");
        assert!(text.is_char_boundary(l.end_offset));
    }
}
