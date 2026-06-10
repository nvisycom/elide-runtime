//! [`PatternRecognizer`]: compiles a [`PatternRegistry`] into pooled
//! scanners and implements [`EntityRecognizer<Text>`].
//!
//! The internal split is intentional: regex patterns go into a
//! single [`regex::RegexSet`] for a one-pass scan across every
//! regex; dictionary terms go into a single
//! [`aho_corasick::AhoCorasick`] automaton for a one-pass scan
//! across every literal. Both passes share one walk over the input
//! and emit entities in modality-local byte coordinates.

use std::sync::Arc;

use aho_corasick::{AhoCorasick, MatchKind};
use nvisy_core::entity::{Entity, EntityKind, PatternProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::{Text, TextLocation};
use nvisy_core::primitive::{Confidence, LanguageTag};
use nvisy_core::recognition::{EntityRecognizer, RecognizerInput, RecognizerOutput};
use nvisy_core::{Error, Result};
use regex::{Regex, RegexSet};

use super::registry::PatternRegistry;
use crate::validators::{Validator, ValidatorRegistry};

/// Source of truth for one runtime pattern: the regex compiled
/// once, plus the metadata needed to emit entities.
///
/// `context` is intentionally not stored on the compiled state —
/// the recognizer never reads it; the [`ContextEnhancer`] looks it
/// up directly on the [`PatternRegistry`] at boost time.
///
/// [`ContextEnhancer`]: crate::ContextEnhancer
struct CompiledPattern {
    name: String,
    entity_kind: EntityKind,
    regex: Regex,
    raw_regex: String,
    score: Confidence,
    validator: Option<Arc<dyn Validator>>,
    /// Languages this pattern applies to. Empty means "any language".
    languages: Vec<LanguageTag>,
}

/// Source of truth for one runtime dictionary: its term range
/// inside the shared Aho-Corasick automaton, plus per-dictionary
/// emission metadata.
struct CompiledDictionary {
    name: String,
    entity_kind: EntityKind,
    /// First term-id (inclusive) for this dictionary inside the
    /// shared automaton.
    term_start: usize,
    /// One past the last term-id for this dictionary inside the
    /// shared automaton.
    term_end: usize,
    /// Per-term confidence, indexed by `term_id - term_start`.
    /// Resolved at compile time from the dictionary's
    /// `column_scores` override (when set) or its default `score`.
    term_scores: Vec<Confidence>,
    /// Languages this dictionary applies to. Empty means "any
    /// language".
    languages: Vec<LanguageTag>,
    /// Reject matches whose immediate neighbours are word
    /// characters (alphanumeric or `_`). Mirrors regex `\b`.
    word_boundary: bool,
}

/// Composes a [`PatternRegistry`] into a single text recognizer.
pub struct PatternRecognizer {
    patterns: Vec<CompiledPattern>,
    regex_set: Option<RegexSet>,
    dictionaries: Vec<CompiledDictionary>,
    aho: Option<AhoCorasick>,
}

impl PatternRecognizer {
    /// Start assembling a recognizer. Required: a registry, supplied
    /// via [`with_registry`].
    ///
    /// [`with_registry`]: PatternRecognizerBuilder::with_registry
    #[must_use]
    pub fn builder() -> PatternRecognizerBuilder {
        PatternRecognizerBuilder::default()
    }
}

/// Builder for [`PatternRecognizer`].
#[derive(Default)]
pub struct PatternRecognizerBuilder {
    registry: Option<PatternRegistry>,
    validators: Option<ValidatorRegistry>,
}

impl PatternRecognizerBuilder {
    /// Attach the pattern + dictionary registry to compile.
    #[must_use]
    pub fn with_registry(mut self, registry: PatternRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Override the validator registry. When unset, the built-in
    /// registry ([`ValidatorRegistry::builtin`]) is used.
    #[must_use]
    pub fn with_validators(mut self, registry: ValidatorRegistry) -> Self {
        self.validators = Some(registry);
        self
    }

    /// Compile every registered pattern and dictionary into the
    /// pooled scanners.
    ///
    /// # Errors
    ///
    /// Returns an error when no registry was supplied, when a
    /// pattern's regex fails to compile, when a pattern references
    /// an unknown validator name, or when the shared automata
    /// cannot be constructed.
    pub fn build(self) -> Result<PatternRecognizer> {
        let registry = self.registry.ok_or_else(|| {
            Error::validation(
                "PatternRecognizer requires a registry — call `with_registry` first",
                "nvisy-pattern",
            )
        })?;
        let validators = self.validators.unwrap_or_else(ValidatorRegistry::builtin);
        let mut compiled_patterns = Vec::with_capacity(registry.patterns().len());
        let mut regex_sources = Vec::with_capacity(registry.patterns().len());

        for pattern in registry.patterns() {
            let regex = Regex::new(&pattern.regex).map_err(|e| {
                Error::validation(
                    format!("pattern `{}`: invalid regex: {e}", pattern.name),
                    "nvisy-pattern",
                )
            })?;
            let validator = match pattern.validator.as_deref() {
                None => None,
                Some(name) => Some(validators.resolve(name).ok_or_else(|| {
                    Error::validation(
                        format!("pattern `{}`: unknown validator `{}`", pattern.name, name),
                        "nvisy-pattern",
                    )
                })?),
            };
            regex_sources.push(pattern.regex.clone());
            compiled_patterns.push(CompiledPattern {
                name: pattern.name.clone(),
                entity_kind: pattern.entity_kind,
                regex,
                raw_regex: pattern.regex.clone(),
                score: pattern.score,
                validator,
                languages: pattern.languages.clone(),
            });
        }

        let regex_set = if regex_sources.is_empty() {
            None
        } else {
            Some(RegexSet::new(&regex_sources).map_err(|e| {
                Error::validation(format!("compiling regex set: {e}"), "nvisy-pattern")
            })?)
        };

        let mut compiled_dicts = Vec::with_capacity(registry.dictionaries().len());
        let mut all_terms: Vec<String> = Vec::new();
        for dict in registry.dictionaries() {
            let term_start = all_terms.len();
            let mut term_scores = Vec::with_capacity(dict.terms.len());
            for entry in dict.terms.entries() {
                all_terms.push(entry.term.clone());
                // Resolve column → score. Out-of-range columns fall
                // back to the dictionary's default score.
                let score = dict
                    .column_scores
                    .get(entry.column as usize)
                    .copied()
                    .unwrap_or(dict.score);
                term_scores.push(score);
            }
            let term_end = all_terms.len();
            compiled_dicts.push(CompiledDictionary {
                name: dict.name.clone(),
                entity_kind: dict.entity_kind,
                term_start,
                term_end,
                term_scores,
                languages: dict.languages.clone(),
                word_boundary: dict.word_boundary,
            });
        }

        let aho = if all_terms.is_empty() {
            None
        } else {
            Some(
                AhoCorasick::builder()
                    .ascii_case_insensitive(false)
                    // Longest-match-at-position: when both `en` and
                    // `English` start at the same offset, return
                    // `English`. Without this, the short ISO code
                    // would win and word-boundary post-filtering
                    // would then reject it, dropping the legitimate
                    // long-form match.
                    .match_kind(MatchKind::LeftmostLongest)
                    .build(&all_terms)
                    .map_err(|e| {
                        Error::validation(
                            format!("compiling dictionary automaton: {e}"),
                            "nvisy-pattern",
                        )
                    })?,
            )
        };

        Ok(PatternRecognizer {
            patterns: compiled_patterns,
            regex_set,
            dictionaries: compiled_dicts,
            aho,
        })
    }
}

#[async_trait::async_trait]
impl EntityRecognizer<Text> for PatternRecognizer {
    async fn recognize(&self, input: &RecognizerInput<Text>) -> Result<RecognizerOutput<Text>> {
        let text = input.data.text.as_str();
        let mut entities = Vec::new();

        if let Some(set) = self.regex_set.as_ref() {
            for pattern_id in set.matches(text).into_iter() {
                let pat = &self.patterns[pattern_id];
                if !input.applies_to_language(&pat.languages) {
                    continue;
                }
                for m in pat.regex.find_iter(text) {
                    if let Some(validator) = pat.validator.as_ref()
                        && !validator.validate(m.as_str())
                    {
                        continue;
                    }
                    entities.push(build_pattern_entity(pat, m.start(), m.end()));
                }
            }
        }

        if let Some(aho) = self.aho.as_ref() {
            for mat in aho.find_iter(text) {
                let term_id = mat.pattern().as_usize();
                let Some(dict) = self.dictionary_owning_term(term_id) else {
                    continue;
                };
                if !input.applies_to_language(&dict.languages) {
                    continue;
                }
                if dict.word_boundary && !has_word_boundaries(text, mat.start(), mat.end()) {
                    continue;
                }
                let score = dict.term_scores[term_id - dict.term_start];
                entities.push(build_dictionary_entity(dict, score, mat.start(), mat.end()));
            }
        }

        Ok(RecognizerOutput::new(entities))
    }
}

impl PatternRecognizer {
    fn dictionary_owning_term(&self, term_id: usize) -> Option<&CompiledDictionary> {
        self.dictionaries
            .iter()
            .find(|d| term_id >= d.term_start && term_id < d.term_end)
    }
}

fn build_pattern_entity(pat: &CompiledPattern, start: usize, end: usize) -> Entity<Text> {
    let provenance = TrailProvenance::Pattern(PatternProvenance::Regex {
        name: pat.name.clone(),
        regex: Some(pat.raw_regex.clone()),
        validator: pat.validator.as_ref().map(|_| pat.name.clone()),
        contextual: false,
    });
    let step = TrailStep::recognition(
        "pattern",
        pat.score,
        provenance,
        format!("pattern `{}` matched", pat.name),
    );
    Entity::builder()
        .with_entity_kind(pat.entity_kind)
        .with_trail(vec![step])
        .with_confidence(pat.score)
        .with_location(TextLocation::new(start, end))
        .build()
        .expect("required fields provided")
}

/// Mirror of regex `\b` for the byte range `text[start..end]`:
/// the immediate neighbour characters (or start/end of input) must
/// not be word characters. A word character here is Unicode
/// alphanumeric or `_`, matching the conventional regex definition.
///
/// Operates on `char` boundaries, not raw bytes, so multibyte
/// codepoints don't trigger false rejections (`é` is one char, not
/// two).
fn has_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let left_is_word = text[..start].chars().next_back().is_some_and(is_word_char);
    let right_is_word = text[end..].chars().next().is_some_and(is_word_char);
    !left_is_word && !right_is_word
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn build_dictionary_entity(
    dict: &CompiledDictionary,
    score: Confidence,
    start: usize,
    end: usize,
) -> Entity<Text> {
    let provenance = TrailProvenance::Pattern(PatternProvenance::Dictionary {
        name: dict.name.clone(),
        contextual: false,
    });
    let step = TrailStep::recognition(
        "pattern",
        score,
        provenance,
        format!("dictionary `{}` matched", dict.name),
    );
    Entity::builder()
        .with_entity_kind(dict.entity_kind)
        .with_trail(vec![step])
        .with_confidence(score)
        .with_location(TextLocation::new(start, end))
        .build()
        .expect("required fields provided")
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::EntityKind;
    use nvisy_core::modality::TextData;
    use nvisy_core::recognition::RecognizerInput;

    use super::*;
    use crate::Dictionary;
    use crate::recognition::registry::PatternRegistry;
    use crate::recognition::terms::Terms;

    fn dict(name: &str, terms: &[&str], word_boundary: bool) -> Dictionary {
        Dictionary::builder()
            .with_name(name.to_owned())
            .with_entity_kind(EntityKind::Language)
            .with_terms(Terms::from(terms))
            .with_word_boundary(word_boundary)
            .build()
            .expect("dictionary builds")
    }

    async fn run(recognizer: &PatternRecognizer, text: &str) -> Vec<Entity<Text>> {
        let input = RecognizerInput::new(TextData::new(text.to_owned()));
        recognizer
            .recognize(&input)
            .await
            .expect("recognize succeeds")
            .entities
    }

    #[tokio::test]
    async fn word_boundary_rejects_substring_matches() {
        let registry = PatternRegistry::new().with_dictionary(dict("langs", &["am", "or"], true));
        let recognizer = PatternRecognizer::builder()
            .with_registry(registry)
            .build()
            .expect("recognizer builds");

        let entities = run(&recognizer, "the example or a candidate").await;
        let matched: Vec<&str> = entities
            .iter()
            .map(|e| &"the example or a candidate"[e.location.start..e.location.end])
            .collect();

        // "am" inside "example" and "or" inside "candidate" are
        // substring matches and must be rejected. The standalone
        // "or" between two spaces must be kept.
        assert_eq!(matched, vec!["or"]);
    }

    #[tokio::test]
    async fn word_boundary_disabled_keeps_substring_matches() {
        let registry = PatternRegistry::new().with_dictionary(dict("langs", &["am"], false));
        let recognizer = PatternRecognizer::builder()
            .with_registry(registry)
            .build()
            .expect("recognizer builds");

        let entities = run(&recognizer, "example").await;
        assert_eq!(entities.len(), 1, "substring match must be kept");
    }

    #[test]
    fn has_word_boundaries_handles_edges_and_unicode() {
        // Match touches both edges of the input → boundaries OK.
        assert!(has_word_boundaries("hello", 0, 5));
        // Match preceded by a word char → not a boundary.
        assert!(!has_word_boundaries("example", 5, 7));
        // Match followed by a word char → not a boundary.
        assert!(!has_word_boundaries("amount", 0, 2));
        // Space surround → boundaries OK.
        assert!(has_word_boundaries(" am ", 1, 3));
        // Unicode word char on the left → not a boundary.
        assert!(!has_word_boundaries("café_am", 5, 7));
        // Punctuation around → boundaries OK.
        assert!(has_word_boundaries("(am)", 1, 3));
    }
}
